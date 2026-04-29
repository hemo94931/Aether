CREATE OR REPLACE FUNCTION public.aether_canonical_api_format_alias(value text)
RETURNS text
LANGUAGE sql
IMMUTABLE
AS $$
  SELECT CASE LOWER(BTRIM(COALESCE(value, '')))
    WHEN 'openai:cli' THEN 'openai:responses'
    WHEN 'openai:compact' THEN 'openai:responses:compact'
    WHEN 'claude:chat' THEN 'claude:messages'
    WHEN 'claude:cli' THEN 'claude:messages'
    WHEN 'gemini:chat' THEN 'gemini:generate_content'
    WHEN 'gemini:cli' THEN 'gemini:generate_content'
    ELSE LOWER(BTRIM(COALESCE(value, '')))
  END
$$;

DO $$
DECLARE
  conflict_summary text;
BEGIN
  SELECT string_agg(
    DISTINCT provider_id::text || ':' || canonical_api_format,
    ', ' ORDER BY provider_id::text || ':' || canonical_api_format
  )
  INTO conflict_summary
  FROM (
    SELECT
      left_endpoint.provider_id,
      public.aether_canonical_api_format_alias(left_endpoint.api_format) AS canonical_api_format
    FROM public.provider_endpoints AS left_endpoint
    INNER JOIN public.provider_endpoints AS right_endpoint
      ON right_endpoint.provider_id = left_endpoint.provider_id
     AND right_endpoint.id > left_endpoint.id
     AND public.aether_canonical_api_format_alias(right_endpoint.api_format)
       = public.aether_canonical_api_format_alias(left_endpoint.api_format)
    WHERE left_endpoint.api_format IN ('openai:responses', 'openai:cli', 'openai:responses:compact', 'openai:compact', 'claude:messages', 'claude:chat', 'claude:cli', 'gemini:generate_content', 'gemini:chat', 'gemini:cli')
      AND right_endpoint.api_format IN ('openai:responses', 'openai:cli', 'openai:responses:compact', 'openai:compact', 'claude:messages', 'claude:chat', 'claude:cli', 'gemini:generate_content', 'gemini:chat', 'gemini:cli')
      AND (
        left_endpoint.base_url IS DISTINCT FROM right_endpoint.base_url
        OR left_endpoint.custom_path IS DISTINCT FROM right_endpoint.custom_path
        OR left_endpoint.max_retries IS DISTINCT FROM right_endpoint.max_retries
        OR left_endpoint.header_rules::jsonb IS DISTINCT FROM right_endpoint.header_rules::jsonb
        OR left_endpoint.body_rules::jsonb IS DISTINCT FROM right_endpoint.body_rules::jsonb
        OR left_endpoint.config::jsonb IS DISTINCT FROM right_endpoint.config::jsonb
        OR left_endpoint.proxy IS DISTINCT FROM right_endpoint.proxy
        OR left_endpoint.format_acceptance_config::jsonb IS DISTINCT FROM right_endpoint.format_acceptance_config::jsonb
      )
  ) AS conflicts;

  IF conflict_summary IS NOT NULL THEN
    RAISE EXCEPTION
      'Cannot normalize OpenAI/Claude/Gemini provider_endpoints because transport fields differ for: %',
      conflict_summary;
  END IF;
END $$;

WITH grouped AS (
  SELECT
    id,
    provider_id,
    api_format,
    public.aether_canonical_api_format_alias(api_format) AS canonical_api_format,
    ROW_NUMBER() OVER (
      PARTITION BY provider_id, public.aether_canonical_api_format_alias(api_format)
      ORDER BY
        CASE
          WHEN api_format = public.aether_canonical_api_format_alias(api_format) THEN 0
          ELSE 1
        END,
        created_at ASC,
        id ASC
    ) AS rank
  FROM public.provider_endpoints
  WHERE api_format IN ('openai:responses', 'openai:cli', 'openai:responses:compact', 'openai:compact', 'claude:messages', 'claude:chat', 'claude:cli', 'gemini:generate_content', 'gemini:chat', 'gemini:cli')
),
survivors AS (
  SELECT *
  FROM grouped
  WHERE rank = 1
),
retired AS (
  UPDATE public.provider_endpoints AS endpoint
  SET
    is_active = FALSE,
    updated_at = NOW()
  FROM grouped
  WHERE endpoint.id = grouped.id
    AND grouped.rank > 1
  RETURNING endpoint.id
)
UPDATE public.provider_endpoints AS endpoint
SET
  api_format = survivors.canonical_api_format,
  api_family = SPLIT_PART(survivors.canonical_api_format, ':', 1),
  endpoint_kind = SUBSTRING(survivors.canonical_api_format FROM POSITION(':' IN survivors.canonical_api_format) + 1),
  updated_at = NOW()
FROM survivors
WHERE endpoint.id = survivors.id
  AND endpoint.api_format IS DISTINCT FROM survivors.canonical_api_format;

WITH expanded AS (
  SELECT
    pak.id,
    formats.ordinality,
    public.aether_canonical_api_format_alias(formats.value) AS api_format
  FROM public.provider_api_keys AS pak
  CROSS JOIN LATERAL json_array_elements_text(
    CASE
      WHEN json_typeof(pak.api_formats) = 'array' THEN pak.api_formats
      ELSE '[]'::json
    END
  ) WITH ORDINALITY AS formats(value, ordinality)
  WHERE pak.api_formats IS NOT NULL
),
deduped AS (
  SELECT id, api_format, MIN(ordinality) AS first_ordinality
  FROM expanded
  WHERE api_format <> ''
  GROUP BY id, api_format
),
rebuilt AS (
  SELECT id, json_agg(api_format ORDER BY first_ordinality) AS api_formats
  FROM deduped
  GROUP BY id
)
UPDATE public.provider_api_keys AS pak
SET
  api_formats = rebuilt.api_formats,
  updated_at = NOW()
FROM rebuilt
WHERE pak.id = rebuilt.id
  AND pak.api_formats::jsonb IS DISTINCT FROM rebuilt.api_formats::jsonb;

WITH expanded AS (
  SELECT
    key.id,
    formats.ordinality,
    public.aether_canonical_api_format_alias(formats.value) AS api_format
  FROM public.api_keys AS key
  CROSS JOIN LATERAL json_array_elements_text(
    CASE
      WHEN json_typeof(key.allowed_api_formats) = 'array' THEN key.allowed_api_formats
      ELSE '[]'::json
    END
  ) WITH ORDINALITY AS formats(value, ordinality)
  WHERE key.allowed_api_formats IS NOT NULL
),
deduped AS (
  SELECT id, api_format, MIN(ordinality) AS first_ordinality
  FROM expanded
  WHERE api_format <> ''
  GROUP BY id, api_format
),
rebuilt AS (
  SELECT id, json_agg(api_format ORDER BY first_ordinality) AS allowed_api_formats
  FROM deduped
  GROUP BY id
)
UPDATE public.api_keys AS key
SET
  allowed_api_formats = rebuilt.allowed_api_formats,
  updated_at = NOW()
FROM rebuilt
WHERE key.id = rebuilt.id
  AND key.allowed_api_formats::jsonb IS DISTINCT FROM rebuilt.allowed_api_formats::jsonb;

WITH expanded AS (
  SELECT
    users.id,
    formats.ordinality,
    public.aether_canonical_api_format_alias(formats.value) AS api_format
  FROM public.users AS users
  CROSS JOIN LATERAL json_array_elements_text(
    CASE
      WHEN json_typeof(users.allowed_api_formats) = 'array' THEN users.allowed_api_formats
      ELSE '[]'::json
    END
  ) WITH ORDINALITY AS formats(value, ordinality)
  WHERE users.allowed_api_formats IS NOT NULL
),
deduped AS (
  SELECT id, api_format, MIN(ordinality) AS first_ordinality
  FROM expanded
  WHERE api_format <> ''
  GROUP BY id, api_format
),
rebuilt AS (
  SELECT id, json_agg(api_format ORDER BY first_ordinality) AS allowed_api_formats
  FROM deduped
  GROUP BY id
)
UPDATE public.users AS users
SET
  allowed_api_formats = rebuilt.allowed_api_formats,
  updated_at = NOW()
FROM rebuilt
WHERE users.id = rebuilt.id
  AND users.allowed_api_formats::jsonb IS DISTINCT FROM rebuilt.allowed_api_formats::jsonb;

WITH mapping_items AS (
  SELECT
    models.id,
    item.ordinality AS item_ordinality,
    item.value AS item
  FROM public.models AS models
  CROSS JOIN LATERAL jsonb_array_elements(
    CASE
      WHEN jsonb_typeof(models.provider_model_mappings) = 'array' THEN models.provider_model_mappings
      ELSE '[]'::jsonb
    END
  ) WITH ORDINALITY AS item(value, ordinality)
  WHERE models.provider_model_mappings IS NOT NULL
),
rebuilt_items AS (
  SELECT
    id,
    item_ordinality,
    CASE
      WHEN jsonb_typeof(item) = 'object'
        AND jsonb_typeof(item->'api_formats') = 'array'
      THEN jsonb_set(
        item,
        '{api_formats}',
        COALESCE(
          (
            SELECT jsonb_agg(api_format ORDER BY first_ordinality)
            FROM (
              SELECT
                public.aether_canonical_api_format_alias(format.value) AS api_format,
                MIN(format.ordinality) AS first_ordinality
              FROM jsonb_array_elements_text(item->'api_formats') WITH ORDINALITY AS format(value, ordinality)
              GROUP BY public.aether_canonical_api_format_alias(format.value)
            ) AS deduped_formats
            WHERE api_format <> ''
          ),
          '[]'::jsonb
        ),
        true
      )
      ELSE item
    END AS item
  FROM mapping_items
),
rebuilt AS (
  SELECT id, jsonb_agg(item ORDER BY item_ordinality) AS provider_model_mappings
  FROM rebuilt_items
  GROUP BY id
)
UPDATE public.models AS models
SET
  provider_model_mappings = rebuilt.provider_model_mappings,
  updated_at = NOW()
FROM rebuilt
WHERE models.id = rebuilt.id
  AND models.provider_model_mappings IS DISTINCT FROM rebuilt.provider_model_mappings;

WITH rebuilt AS (
  SELECT
    pak.id,
    jsonb_object_agg(
      public.aether_canonical_api_format_alias(entry.key),
      entry.value
      ORDER BY entry.ordinality
    ) AS rate_multipliers
  FROM public.provider_api_keys AS pak
  CROSS JOIN LATERAL json_each(
    CASE
      WHEN json_typeof(pak.rate_multipliers) = 'object' THEN pak.rate_multipliers
      ELSE '{}'::json
    END
  ) WITH ORDINALITY AS entry(key, value, ordinality)
  WHERE pak.rate_multipliers IS NOT NULL
  GROUP BY pak.id
)
UPDATE public.provider_api_keys AS pak
SET
  rate_multipliers = rebuilt.rate_multipliers::json,
  updated_at = NOW()
FROM rebuilt
WHERE pak.id = rebuilt.id
  AND pak.rate_multipliers::jsonb IS DISTINCT FROM rebuilt.rate_multipliers;

WITH rebuilt AS (
  SELECT
    pak.id,
    jsonb_object_agg(
      public.aether_canonical_api_format_alias(entry.key),
      entry.value
      ORDER BY entry.ordinality
    ) AS global_priority_by_format
  FROM public.provider_api_keys AS pak
  CROSS JOIN LATERAL json_each(
    CASE
      WHEN json_typeof(pak.global_priority_by_format) = 'object' THEN pak.global_priority_by_format
      ELSE '{}'::json
    END
  ) WITH ORDINALITY AS entry(key, value, ordinality)
  WHERE pak.global_priority_by_format IS NOT NULL
  GROUP BY pak.id
)
UPDATE public.provider_api_keys AS pak
SET
  global_priority_by_format = rebuilt.global_priority_by_format::json,
  updated_at = NOW()
FROM rebuilt
WHERE pak.id = rebuilt.id
  AND pak.global_priority_by_format::jsonb IS DISTINCT FROM rebuilt.global_priority_by_format;

WITH rebuilt AS (
  SELECT
    pak.id,
    jsonb_object_agg(
      public.aether_canonical_api_format_alias(entry.key),
      entry.value
      ORDER BY entry.ordinality
    ) AS health_by_format
  FROM public.provider_api_keys AS pak
  CROSS JOIN LATERAL jsonb_each(
    CASE
      WHEN jsonb_typeof(pak.health_by_format) = 'object' THEN pak.health_by_format
      ELSE '{}'::jsonb
    END
  ) WITH ORDINALITY AS entry(key, value, ordinality)
  WHERE pak.health_by_format IS NOT NULL
  GROUP BY pak.id
)
UPDATE public.provider_api_keys AS pak
SET
  health_by_format = rebuilt.health_by_format,
  updated_at = NOW()
FROM rebuilt
WHERE pak.id = rebuilt.id
  AND pak.health_by_format IS DISTINCT FROM rebuilt.health_by_format;

WITH rebuilt AS (
  SELECT
    pak.id,
    jsonb_object_agg(
      public.aether_canonical_api_format_alias(entry.key),
      entry.value
      ORDER BY entry.ordinality
    ) AS circuit_breaker_by_format
  FROM public.provider_api_keys AS pak
  CROSS JOIN LATERAL jsonb_each(
    CASE
      WHEN jsonb_typeof(pak.circuit_breaker_by_format) = 'object' THEN pak.circuit_breaker_by_format
      ELSE '{}'::jsonb
    END
  ) WITH ORDINALITY AS entry(key, value, ordinality)
  WHERE pak.circuit_breaker_by_format IS NOT NULL
  GROUP BY pak.id
)
UPDATE public.provider_api_keys AS pak
SET
  circuit_breaker_by_format = rebuilt.circuit_breaker_by_format,
  updated_at = NOW()
FROM rebuilt
WHERE pak.id = rebuilt.id
  AND pak.circuit_breaker_by_format IS DISTINCT FROM rebuilt.circuit_breaker_by_format;

DROP FUNCTION public.aether_canonical_api_format_alias(text);
