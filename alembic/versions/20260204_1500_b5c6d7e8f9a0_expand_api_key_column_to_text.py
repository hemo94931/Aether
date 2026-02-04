"""Add provider_type and expand string columns to TEXT

- Add providers.provider_type (String(20), server_default="custom")
- Expand multiple VARCHAR columns to TEXT for long values (OAuth tokens, LDAP DN, URLs, etc.)

Revision ID: b5c6d7e8f9a0
Revises: c4e8f9a1b2c3
Create Date: 2026-02-04 15:00:00.000000

"""

from typing import Sequence, Union

import sqlalchemy as sa
from alembic import op
from sqlalchemy import inspect

# revision identifiers, used by Alembic.
revision: str = "b5c6d7e8f9a0"
down_revision: Union[str, None] = "c4e8f9a1b2c3"
branch_labels: Union[str, Sequence[str], None] = None
depends_on: Union[str, Sequence[str], None] = None


# 需要扩展为 TEXT 的列（表名, 列名, 原始类型长度）
COLUMNS_TO_EXPAND = [
    ("provider_api_keys", "api_key", 500),  # OAuth tokens can be very long
    ("ldap_configs", "bind_dn", 255),  # LDAP DN can be deeply nested
    ("ldap_configs", "base_dn", 255),  # LDAP DN can be deeply nested
    ("ldap_configs", "user_search_filter", 500),  # Complex LDAP filters
    ("oauth_providers", "client_id", 255),  # Some OAuth providers use JWT client_id
]


def column_exists(table_name: str, column_name: str) -> bool:
    """检查列是否已存在"""
    bind = op.get_bind()
    inspector = inspect(bind)
    columns = [col["name"] for col in inspector.get_columns(table_name)]
    return column_name in columns


def table_exists(table_name: str) -> bool:
    """检查表是否存在"""
    bind = op.get_bind()
    inspector = inspect(bind)
    return table_name in inspector.get_table_names()


def is_sqlite() -> bool:
    """检查是否为 SQLite 数据库"""
    bind = op.get_bind()
    return bind.dialect.name == "sqlite"


def expand_column_to_text(table_name: str, column_name: str, original_length: int) -> None:
    """将 VARCHAR 列扩展为 TEXT（兼容 SQLite）"""
    if not table_exists(table_name):
        return
    if not column_exists(table_name, column_name):
        return

    if is_sqlite():
        # SQLite 不支持直接 ALTER COLUMN，需要用 batch 模式
        with op.batch_alter_table(table_name) as batch_op:
            batch_op.alter_column(
                column_name,
                type_=sa.Text(),
                existing_type=sa.String(original_length),
            )
    else:
        op.alter_column(
            table_name,
            column_name,
            type_=sa.Text(),
            existing_type=sa.String(original_length),
        )


def shrink_column_to_varchar(
    table_name: str, column_name: str, target_length: int, nullable: bool = False
) -> None:
    """将 TEXT 列缩小为 VARCHAR（兼容 SQLite）
    WARNING: 如果数据超过 target_length 会失败
    """
    if not table_exists(table_name):
        return
    if not column_exists(table_name, column_name):
        return

    if is_sqlite():
        with op.batch_alter_table(table_name) as batch_op:
            batch_op.alter_column(
                column_name,
                type_=sa.String(target_length),
                existing_type=sa.Text(),
                existing_nullable=nullable,
            )
    else:
        op.alter_column(
            table_name,
            column_name,
            type_=sa.String(target_length),
            existing_type=sa.Text(),
            existing_nullable=nullable,
        )


def upgrade() -> None:
    # Add providers.provider_type
    if not column_exists("providers", "provider_type"):
        op.add_column(
            "providers",
            sa.Column("provider_type", sa.String(20), nullable=False, server_default="custom"),
        )

    # Expand VARCHAR columns to TEXT
    for table_name, column_name, original_length in COLUMNS_TO_EXPAND:
        expand_column_to_text(table_name, column_name, original_length)


def downgrade() -> None:
    # Shrink TEXT columns back to VARCHAR
    # WARNING: Downgrade may fail if any values exceed original length
    for table_name, column_name, original_length in reversed(COLUMNS_TO_EXPAND):
        shrink_column_to_varchar(table_name, column_name, original_length)

    # Drop providers.provider_type
    if column_exists("providers", "provider_type"):
        op.drop_column("providers", "provider_type")
