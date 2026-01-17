"""
Provider 架构模块
"""

from src.services.provider_ops.architectures.base import (
    ProviderArchitecture,
    ProviderConnector,
    VerifyResult,
)
from src.services.provider_ops.architectures.generic_api import GenericApiArchitecture
from src.services.provider_ops.architectures.new_api import NewApiArchitecture
from src.services.provider_ops.architectures.one_api import OneApiArchitecture

__all__ = [
    "ProviderArchitecture",
    "ProviderConnector",
    "VerifyResult",
    "GenericApiArchitecture",
    "NewApiArchitecture",
    "OneApiArchitecture",
]
