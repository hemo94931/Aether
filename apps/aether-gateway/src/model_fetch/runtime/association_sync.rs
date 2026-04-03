use std::collections::BTreeSet;

use aether_data::repository::global_models::{
    AdminGlobalModelListQuery, AdminProviderModelListQuery, UpsertAdminProviderModelRecord,
};
use serde_json::Value;
use uuid::Uuid;

use crate::gateway::{AppState, GatewayError};

pub(super) async fn sync_provider_model_whitelist_associations(
    state: &AppState,
    provider_id: &str,
    current_allowed_models: &[String],
) -> Result<(), GatewayError> {
    if !state.data.has_global_model_reader() || !state.data.has_global_model_writer() {
        return Ok(());
    }

    auto_associate_provider_by_key_whitelist(state, provider_id, current_allowed_models).await?;
    auto_disassociate_provider_by_key_whitelist(state, provider_id).await?;
    Ok(())
}

async fn auto_associate_provider_by_key_whitelist(
    state: &AppState,
    provider_id: &str,
    allowed_models: &[String],
) -> Result<(), GatewayError> {
    if allowed_models.is_empty() {
        return Ok(());
    }

    let provider_models = state
        .list_admin_provider_models(&AdminProviderModelListQuery {
            provider_id: provider_id.to_string(),
            is_active: None,
            offset: 0,
            limit: 10_000,
        })
        .await?;
    let linked_global_model_ids = provider_models
        .iter()
        .map(|model| model.global_model_id.clone())
        .collect::<BTreeSet<_>>();
    let existing_provider_model_names = provider_models
        .iter()
        .map(|model| model.provider_model_name.clone())
        .collect::<BTreeSet<_>>();
    let global_models = state
        .list_admin_global_models(&AdminGlobalModelListQuery {
            offset: 0,
            limit: 10_000,
            is_active: Some(true),
            search: None,
        })
        .await?
        .items;

    for global_model in global_models {
        if linked_global_model_ids.contains(&global_model.id)
            || existing_provider_model_names.contains(&global_model.name)
        {
            continue;
        }

        let mappings = global_model_mapping_patterns(global_model.config.as_ref());
        if mappings.is_empty() {
            continue;
        }
        if !allowed_models.iter().any(|allowed_model| {
            mappings
                .iter()
                .any(|pattern| matches_model_mapping(pattern, allowed_model))
        }) {
            continue;
        }

        let record = UpsertAdminProviderModelRecord::new(
            Uuid::new_v4().to_string(),
            provider_id.to_string(),
            global_model.id.clone(),
            global_model.name.clone(),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            true,
            true,
            None,
        )
        .map_err(|err| GatewayError::Internal(err.to_string()))?;
        state.create_admin_provider_model(&record).await?;
    }

    Ok(())
}

async fn auto_disassociate_provider_by_key_whitelist(
    state: &AppState,
    provider_id: &str,
) -> Result<(), GatewayError> {
    let keys = state
        .list_provider_catalog_keys_by_provider_ids(&[provider_id.to_string()])
        .await?;
    let active_non_oauth_keys = keys
        .into_iter()
        .filter(|key| key.is_active)
        .filter(|key| !is_oauth_auth_type(&key.auth_type))
        .collect::<Vec<_>>();
    if active_non_oauth_keys.is_empty() {
        return Ok(());
    }
    if active_non_oauth_keys
        .iter()
        .any(|key| key.allowed_models.is_none())
    {
        return Ok(());
    }

    let all_allowed_models = active_non_oauth_keys
        .iter()
        .flat_map(|key| super::json_string_list(key.allowed_models.as_ref()))
        .collect::<BTreeSet<_>>();
    let provider_models = state
        .list_admin_provider_models(&AdminProviderModelListQuery {
            provider_id: provider_id.to_string(),
            is_active: None,
            offset: 0,
            limit: 10_000,
        })
        .await?;

    for model in provider_models {
        let mappings = global_model_mapping_patterns(model.global_model_config.as_ref());
        if mappings.is_empty() {
            continue;
        }
        let matched = all_allowed_models.iter().any(|allowed_model| {
            mappings
                .iter()
                .any(|pattern| matches_model_mapping(pattern, allowed_model))
        });
        if matched {
            continue;
        }
        state
            .delete_admin_provider_model(provider_id, &model.id)
            .await?;
    }

    Ok(())
}

fn global_model_mapping_patterns(config: Option<&Value>) -> Vec<String> {
    config
        .and_then(Value::as_object)
        .and_then(|object| object.get("model_mappings"))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn matches_model_mapping(pattern: &str, model_name: &str) -> bool {
    regex::Regex::new(&format!("^(?:{pattern})$"))
        .ok()
        .is_some_and(|compiled| compiled.is_match(model_name))
}

fn is_oauth_auth_type(value: &str) -> bool {
    matches!(value.trim().to_ascii_lowercase().as_str(), "oauth" | "kiro")
}
