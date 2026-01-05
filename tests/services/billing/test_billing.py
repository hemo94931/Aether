"""
Billing 模块测试

测试计费模块的核心功能：
- BillingCalculator 计费计算
- 计费模板
- 阶梯计费
- calculate_request_cost 便捷函数
"""

import pytest

from src.services.billing import (
    BillingCalculator,
    BillingDimension,
    BillingTemplates,
    BillingUnit,
    CostBreakdown,
    StandardizedUsage,
    calculate_request_cost,
)
from src.services.billing.templates import get_template, list_templates


class TestBillingDimension:
    """测试计费维度"""

    def test_calculate_per_1m_tokens(self) -> None:
        """测试 per_1m_tokens 计费"""
        dim = BillingDimension(
            name="input",
            usage_field="input_tokens",
            price_field="input_price_per_1m",
        )

        # 1000 tokens * $3 / 1M = $0.003
        cost = dim.calculate(1000, 3.0)
        assert abs(cost - 0.003) < 0.0001

    def test_calculate_per_request(self) -> None:
        """测试按次计费"""
        dim = BillingDimension(
            name="request",
            usage_field="request_count",
            price_field="price_per_request",
            unit=BillingUnit.PER_REQUEST,
        )

        # 按次计费：cost = request_count * price
        cost = dim.calculate(1, 0.05)
        assert cost == 0.05

        # 多次请求应按次数计费
        cost = dim.calculate(3, 0.05)
        assert abs(cost - 0.15) < 0.0001

    def test_calculate_zero_usage(self) -> None:
        """测试零用量"""
        dim = BillingDimension(
            name="input",
            usage_field="input_tokens",
            price_field="input_price_per_1m",
        )

        cost = dim.calculate(0, 3.0)
        assert cost == 0.0

    def test_calculate_zero_price(self) -> None:
        """测试零价格"""
        dim = BillingDimension(
            name="input",
            usage_field="input_tokens",
            price_field="input_price_per_1m",
        )

        cost = dim.calculate(1000, 0.0)
        assert cost == 0.0

    def test_to_dict_and_from_dict(self) -> None:
        """测试序列化和反序列化"""
        dim = BillingDimension(
            name="cache_read",
            usage_field="cache_read_tokens",
            price_field="cache_read_price_per_1m",
            unit=BillingUnit.PER_1M_TOKENS,
            default_price=0.3,
        )

        d = dim.to_dict()
        restored = BillingDimension.from_dict(d)

        assert restored.name == dim.name
        assert restored.usage_field == dim.usage_field
        assert restored.price_field == dim.price_field
        assert restored.unit == dim.unit
        assert restored.default_price == dim.default_price


class TestStandardizedUsage:
    """测试标准化 Usage"""

    def test_basic_usage(self) -> None:
        """测试基础 usage"""
        usage = StandardizedUsage(
            input_tokens=1000,
            output_tokens=500,
        )

        assert usage.input_tokens == 1000
        assert usage.output_tokens == 500
        assert usage.cache_creation_tokens == 0
        assert usage.cache_read_tokens == 0

    def test_get_field(self) -> None:
        """测试字段获取"""
        usage = StandardizedUsage(
            input_tokens=1000,
            output_tokens=500,
        )

        assert usage.get("input_tokens") == 1000
        assert usage.get("nonexistent", 0) == 0

    def test_extra_fields(self) -> None:
        """测试扩展字段"""
        usage = StandardizedUsage(
            input_tokens=1000,
            output_tokens=500,
            extra={"custom_field": 123},
        )

        assert usage.get("custom_field") == 123

    def test_to_dict(self) -> None:
        """测试转换为字典"""
        usage = StandardizedUsage(
            input_tokens=1000,
            output_tokens=500,
            cache_creation_tokens=100,
        )

        d = usage.to_dict()
        assert d["input_tokens"] == 1000
        assert d["output_tokens"] == 500
        assert d["cache_creation_tokens"] == 100


class TestCostBreakdown:
    """测试费用明细"""

    def test_basic_breakdown(self) -> None:
        """测试基础费用明细"""
        breakdown = CostBreakdown(
            costs={"input": 0.003, "output": 0.0075},
            total_cost=0.0105,
        )

        assert breakdown.input_cost == 0.003
        assert breakdown.output_cost == 0.0075
        assert breakdown.total_cost == 0.0105

    def test_cache_cost_calculation(self) -> None:
        """测试缓存费用汇总"""
        breakdown = CostBreakdown(
            costs={
                "input": 0.003,
                "output": 0.0075,
                "cache_creation": 0.001,
                "cache_read": 0.0005,
            },
            total_cost=0.012,
        )

        # cache_cost = cache_creation + cache_read
        assert abs(breakdown.cache_cost - 0.0015) < 0.0001

    def test_to_dict(self) -> None:
        """测试转换为字典"""
        breakdown = CostBreakdown(
            costs={"input": 0.003, "output": 0.0075},
            total_cost=0.0105,
            tier_index=1,
        )

        d = breakdown.to_dict()
        assert d["total_cost"] == 0.0105
        assert d["tier_index"] == 1
        assert d["input_cost"] == 0.003


class TestBillingTemplates:
    """测试计费模板"""

    def test_claude_template(self) -> None:
        """测试 Claude 模板"""
        template = BillingTemplates.CLAUDE_STANDARD
        dim_names = [d.name for d in template]

        assert "input" in dim_names
        assert "output" in dim_names
        assert "cache_creation" in dim_names
        assert "cache_read" in dim_names

    def test_openai_template(self) -> None:
        """测试 OpenAI 模板"""
        template = BillingTemplates.OPENAI_STANDARD
        dim_names = [d.name for d in template]

        assert "input" in dim_names
        assert "output" in dim_names
        assert "cache_read" in dim_names
        # OpenAI 没有缓存创建费用
        assert "cache_creation" not in dim_names

    def test_gemini_template(self) -> None:
        """测试 Gemini 模板"""
        template = BillingTemplates.GEMINI_STANDARD
        dim_names = [d.name for d in template]

        assert "input" in dim_names
        assert "output" in dim_names
        assert "cache_read" in dim_names

    def test_per_request_template(self) -> None:
        """测试按次计费模板"""
        template = BillingTemplates.PER_REQUEST
        assert len(template) == 1
        assert template[0].name == "request"
        assert template[0].unit == BillingUnit.PER_REQUEST

    def test_get_template(self) -> None:
        """测试获取模板"""
        template = get_template("claude")
        assert template == BillingTemplates.CLAUDE_STANDARD

        template = get_template("openai")
        assert template == BillingTemplates.OPENAI_STANDARD

        # 不区分大小写
        template = get_template("CLAUDE")
        assert template == BillingTemplates.CLAUDE_STANDARD

        with pytest.raises(ValueError, match="Unknown billing template"):
            get_template("unknown_template")

    def test_list_templates(self) -> None:
        """测试列出模板"""
        templates = list_templates()

        assert "claude" in templates
        assert "openai" in templates
        assert "gemini" in templates
        assert "per_request" in templates


class TestBillingCalculator:
    """测试计费计算器"""

    def test_basic_calculation(self) -> None:
        """测试基础计费计算"""
        calculator = BillingCalculator(template="claude")
        usage = StandardizedUsage(input_tokens=1000, output_tokens=500)
        prices = {"input_price_per_1m": 3.0, "output_price_per_1m": 15.0}

        result = calculator.calculate(usage, prices)

        # 1000 * 3 / 1M = 0.003
        assert abs(result.input_cost - 0.003) < 0.0001
        # 500 * 15 / 1M = 0.0075
        assert abs(result.output_cost - 0.0075) < 0.0001
        # Total = 0.0105
        assert abs(result.total_cost - 0.0105) < 0.0001

    def test_calculation_with_cache(self) -> None:
        """测试带缓存的计费计算"""
        calculator = BillingCalculator(template="claude")
        usage = StandardizedUsage(
            input_tokens=1000,
            output_tokens=500,
            cache_creation_tokens=200,
            cache_read_tokens=300,
        )
        prices = {
            "input_price_per_1m": 3.0,
            "output_price_per_1m": 15.0,
            "cache_creation_price_per_1m": 3.75,
            "cache_read_price_per_1m": 0.3,
        }

        result = calculator.calculate(usage, prices)

        # cache_creation: 200 * 3.75 / 1M = 0.00075
        assert abs(result.cache_creation_cost - 0.00075) < 0.0001
        # cache_read: 300 * 0.3 / 1M = 0.00009
        assert abs(result.cache_read_cost - 0.00009) < 0.0001

    def test_tiered_pricing(self) -> None:
        """测试阶梯计费"""
        calculator = BillingCalculator(template="claude")
        usage = StandardizedUsage(input_tokens=250000, output_tokens=10000)

        # 大于 200k 进入第二阶梯
        tiered_pricing = {
            "tiers": [
                {"up_to": 200000, "input_price_per_1m": 3.0, "output_price_per_1m": 15.0},
                {"up_to": None, "input_price_per_1m": 1.5, "output_price_per_1m": 7.5},
            ]
        }
        prices = {"input_price_per_1m": 3.0, "output_price_per_1m": 15.0}

        result = calculator.calculate(usage, prices, tiered_pricing)

        # 应该使用第二阶梯价格
        assert result.tier_index == 1
        # 250000 * 1.5 / 1M = 0.375
        assert abs(result.input_cost - 0.375) < 0.0001

    def test_openai_no_cache_creation(self) -> None:
        """测试 OpenAI 模板没有缓存创建费用"""
        calculator = BillingCalculator(template="openai")
        usage = StandardizedUsage(
            input_tokens=1000,
            output_tokens=500,
            cache_creation_tokens=200,  # 这个不应该计费
            cache_read_tokens=300,
        )
        prices = {
            "input_price_per_1m": 3.0,
            "output_price_per_1m": 15.0,
            "cache_creation_price_per_1m": 3.75,
            "cache_read_price_per_1m": 0.3,
        }

        result = calculator.calculate(usage, prices)

        # OpenAI 模板不包含 cache_creation 维度
        assert result.cache_creation_cost == 0.0
        # 但 cache_read 应该计费
        assert result.cache_read_cost > 0

    def test_from_config(self) -> None:
        """测试从配置创建计算器"""
        config = {"template": "openai"}
        calculator = BillingCalculator.from_config(config)

        assert calculator.template_name == "openai"


class TestCalculateRequestCost:
    """测试便捷函数"""

    def test_basic_usage(self) -> None:
        """测试基础用法"""
        result = calculate_request_cost(
            input_tokens=1000,
            output_tokens=500,
            cache_creation_input_tokens=0,
            cache_read_input_tokens=0,
            input_price_per_1m=3.0,
            output_price_per_1m=15.0,
            cache_creation_price_per_1m=None,
            cache_read_price_per_1m=None,
            price_per_request=None,
            billing_template="claude",
        )

        assert "input_cost" in result
        assert "output_cost" in result
        assert "total_cost" in result
        assert abs(result["input_cost"] - 0.003) < 0.0001
        assert abs(result["output_cost"] - 0.0075) < 0.0001

    def test_with_cache(self) -> None:
        """测试带缓存"""
        result = calculate_request_cost(
            input_tokens=1000,
            output_tokens=500,
            cache_creation_input_tokens=200,
            cache_read_input_tokens=300,
            input_price_per_1m=3.0,
            output_price_per_1m=15.0,
            cache_creation_price_per_1m=3.75,
            cache_read_price_per_1m=0.3,
            price_per_request=None,
            billing_template="claude",
        )

        assert result["cache_creation_cost"] > 0
        assert result["cache_read_cost"] > 0
        assert result["cache_cost"] == result["cache_creation_cost"] + result["cache_read_cost"]

    def test_different_templates(self) -> None:
        """测试不同模板"""
        prices = {
            "input_tokens": 1000,
            "output_tokens": 500,
            "cache_creation_input_tokens": 200,
            "cache_read_input_tokens": 300,
            "input_price_per_1m": 3.0,
            "output_price_per_1m": 15.0,
            "cache_creation_price_per_1m": 3.75,
            "cache_read_price_per_1m": 0.3,
            "price_per_request": None,
        }

        # Claude 模板有 cache_creation
        result_claude = calculate_request_cost(**prices, billing_template="claude")
        assert result_claude["cache_creation_cost"] > 0

        # OpenAI 模板没有 cache_creation
        result_openai = calculate_request_cost(**prices, billing_template="openai")
        assert result_openai["cache_creation_cost"] == 0

    def test_tiered_pricing_with_total_context(self) -> None:
        """测试使用自定义 total_input_context 的阶梯计费"""
        tiered_pricing = {
            "tiers": [
                {"up_to": 200000, "input_price_per_1m": 3.0, "output_price_per_1m": 15.0},
                {"up_to": None, "input_price_per_1m": 1.5, "output_price_per_1m": 7.5},
            ]
        }

        # 传入预计算的 total_input_context
        result = calculate_request_cost(
            input_tokens=1000,
            output_tokens=500,
            cache_creation_input_tokens=0,
            cache_read_input_tokens=0,
            input_price_per_1m=3.0,
            output_price_per_1m=15.0,
            cache_creation_price_per_1m=None,
            cache_read_price_per_1m=None,
            price_per_request=None,
            tiered_pricing=tiered_pricing,
            total_input_context=250000,  # 预计算的值，超过 200k
            billing_template="claude",
        )

        # 应该使用第二阶梯价格
        assert result["tier_index"] == 1
