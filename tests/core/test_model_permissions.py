from src.core.model_permissions import check_model_allowed_with_mappings


class TestCheckModelAllowedWithMappings:
    def test_exact_match_returns_allowed_without_mapping(self) -> None:
        is_allowed, matched = check_model_allowed_with_mappings(
            model_name="gpt-4o",
            allowed_models=["gpt-4o"],
            resolved_model_name="gpt-4o",
            model_mappings=[r"gpt-4o-.*"],
        )
        assert is_allowed is True
        assert matched is None

    def test_mapping_match_is_deterministic(self) -> None:
        is_allowed, matched = check_model_allowed_with_mappings(
            model_name="target",
            allowed_models=["b", "a"],
            resolved_model_name="target",
            model_mappings=[r".*"],
        )
        assert is_allowed is True
        assert matched == "a"

    def test_mapping_match_respects_candidate_models(self) -> None:
        is_allowed, matched = check_model_allowed_with_mappings(
            model_name="target",
            allowed_models=["other-1", "allowed-1"],
            resolved_model_name="target",
            model_mappings=[r".*-1"],
            candidate_models={"allowed-1"},
        )
        assert is_allowed is True
        assert matched == "allowed-1"

    def test_mapping_match_candidate_models_no_intersection(self) -> None:
        is_allowed, matched = check_model_allowed_with_mappings(
            model_name="target",
            allowed_models=["allowed-1"],
            resolved_model_name="target",
            model_mappings=[r".*-1"],
            candidate_models={"not-present"},
        )
        assert is_allowed is False
        assert matched is None
