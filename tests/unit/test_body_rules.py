from typing import Any

from src.api.handlers.base.request_builder import apply_body_rules


class TestApplyBodyRulesNestedPaths:
    def test_set_nested_value(self) -> None:
        body = {"a": 1}
        result = apply_body_rules(
            body,
            [
                {"action": "set", "path": "b.c.d", "value": 42},
            ],
        )
        assert result == {"a": 1, "b": {"c": {"d": 42}}}

    def test_drop_nested_value(self) -> None:
        body = {"a": {"b": {"c": 1, "d": 2}}}
        result = apply_body_rules(
            body,
            [
                {"action": "drop", "path": "a.b.c"},
            ],
        )
        assert result == {"a": {"b": {"d": 2}}}

    def test_rename_nested_value(self) -> None:
        body = {"old": {"nested": "value"}}
        result = apply_body_rules(
            body,
            [
                {"action": "rename", "from": "old.nested", "to": "new.path"},
            ],
        )
        assert result == {"old": {}, "new": {"path": "value"}}

    def test_protected_top_level_key(self) -> None:
        body = {"model": "gpt-4", "extra": {"model": "ignored"}}
        result = apply_body_rules(
            body,
            [
                {"action": "set", "path": "model.sub", "value": "x"},  # 应被忽略
                {"action": "set", "path": "extra.model", "value": "y"},  # 应生效
            ],
        )
        assert result["model"] == "gpt-4"  # 顶层受保护，不变
        assert result["extra"]["model"] == "y"  # extra 不受保护

    def test_escaped_dot(self) -> None:
        body = {}
        result = apply_body_rules(
            body,
            [
                {"action": "set", "path": "config\\.v1.enabled", "value": True},
            ],
        )
        assert result == {"config.v1": {"enabled": True}}

    def test_invalid_paths_are_ignored(self) -> None:
        body = {"a": 1}
        result = apply_body_rules(
            body,
            [
                {"action": "set", "path": ".b", "value": 2},
                {"action": "set", "path": "c..d", "value": 3},
                {"action": "drop", "path": "e.", "value": None},
                {"action": "rename", "from": "x..y", "to": "z", "value": None},
            ],
        )
        assert result == {"a": 1}

    def test_non_dict_intermediate_is_overwritten(self) -> None:
        body = {"a": 1}
        result = apply_body_rules(
            body,
            [
                {"action": "set", "path": "a.b", "value": 2},
            ],
        )
        assert result == {"a": {"b": 2}}

    def test_set_complex_value(self) -> None:
        body = {}
        result = apply_body_rules(
            body,
            [
                {"action": "set", "path": "metadata.tags", "value": [1, 2]},
                {"action": "set", "path": "metadata.obj", "value": {"a": 1}},
            ],
        )
        assert result == {"metadata": {"tags": [1, 2], "obj": {"a": 1}}}

    def test_does_not_mutate_original_body(self) -> None:
        body = {"a": {"b": 1}}
        result = apply_body_rules(body, [{"action": "set", "path": "a.c", "value": 2}])
        assert result == {"a": {"b": 1, "c": 2}}
        assert body == {"a": {"b": 1}}


class TestSetWithOriginalPlaceholder:
    """set 操作中 {{$original}} 占位符的测试"""

    def test_wrap_string_in_object(self) -> None:
        """核心用例：字符串值包裹成对象结构"""
        body = {"para": "text"}
        result = apply_body_rules(
            body,
            [{"action": "set", "path": "para", "value": [{"text": "{{$original}}"}]}],
        )
        assert result == {"para": [{"text": "text"}]}

    def test_exact_placeholder_preserves_number(self) -> None:
        """完全匹配占位符时保留原始类型: number"""
        body = {"count": 42}
        result = apply_body_rules(
            body,
            [{"action": "set", "path": "count", "value": {"num": "{{$original}}"}}],
        )
        assert result == {"count": {"num": 42}}
        assert isinstance(result["count"]["num"], int)

    def test_exact_placeholder_preserves_dict(self) -> None:
        """完全匹配占位符时保留原始类型: dict"""
        body = {"data": {"key": "val"}}
        result = apply_body_rules(
            body,
            [{"action": "set", "path": "data", "value": {"wrapped": "{{$original}}"}}],
        )
        assert result == {"data": {"wrapped": {"key": "val"}}}

    def test_exact_placeholder_preserves_list(self) -> None:
        """完全匹配占位符时保留原始类型: list"""
        body = {"items": [1, 2, 3]}
        result = apply_body_rules(
            body,
            [{"action": "set", "path": "items", "value": {"arr": "{{$original}}"}}],
        )
        assert result == {"items": {"arr": [1, 2, 3]}}

    def test_partial_placeholder_converts_to_string(self) -> None:
        """部分匹配时将原值转为 str 拼接"""
        body = {"name": "world"}
        result = apply_body_rules(
            body,
            [{"action": "set", "path": "name", "value": "hello_{{$original}}_suffix"}],
        )
        assert result == {"name": "hello_world_suffix"}

    def test_partial_placeholder_with_number(self) -> None:
        """部分匹配 + 非字符串原值: 数字转 str"""
        body = {"ver": 3}
        result = apply_body_rules(
            body,
            [{"action": "set", "path": "ver", "value": "version_{{$original}}"}],
        )
        assert result == {"ver": "version_3"}

    def test_no_placeholder_acts_like_plain_set(self) -> None:
        """不含占位符时行为与原 set 完全一致"""
        body = {"a": 1}
        result = apply_body_rules(
            body,
            [{"action": "set", "path": "a", "value": {"new": "val"}}],
        )
        assert result == {"a": {"new": "val"}}

    def test_missing_path_uses_none(self) -> None:
        """路径不存在时 original 为 None"""
        body: dict[str, Any] = {}
        result = apply_body_rules(
            body,
            [{"action": "set", "path": "missing", "value": {"was": "{{$original}}"}}],
        )
        assert result == {"missing": {"was": None}}

    def test_nested_path(self) -> None:
        """嵌套路径"""
        body = {"a": {"b": "original"}}
        result = apply_body_rules(
            body,
            [{"action": "set", "path": "a.b", "value": [{"content": "{{$original}}"}]}],
        )
        assert result == {"a": {"b": [{"content": "original"}]}}

    def test_protected_field_ignored(self) -> None:
        """受保护字段（model, stream）跳过"""
        body = {"model": "gpt-4", "other": "val"}
        result = apply_body_rules(
            body,
            [{"action": "set", "path": "model", "value": "{{$original}}_modified"}],
        )
        assert result["model"] == "gpt-4"

    def test_does_not_mutate_original(self) -> None:
        """不修改原始 body"""
        body = {"x": "original"}
        result = apply_body_rules(
            body,
            [{"action": "set", "path": "x", "value": {"wrapped": "{{$original}}"}}],
        )
        assert result == {"x": {"wrapped": "original"}}
        assert body == {"x": "original"}

    def test_multiple_placeholders_in_one_string(self) -> None:
        """一个字符串中多个占位符"""
        body = {"val": "X"}
        result = apply_body_rules(
            body,
            [{"action": "set", "path": "val", "value": "{{$original}}-{{$original}}"}],
        )
        assert result == {"val": "X-X"}

    def test_deeply_nested_template(self) -> None:
        """深层嵌套模板"""
        body = {"data": "hello"}
        result = apply_body_rules(
            body,
            [{"action": "set", "path": "data", "value": {"a": {"b": [{"c": "{{$original}}"}]}}}],
        )
        assert result == {"data": {"a": {"b": [{"c": "hello"}]}}}
