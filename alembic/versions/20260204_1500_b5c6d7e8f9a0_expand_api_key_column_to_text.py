"""Add provider_type and expand api_key column to TEXT

- Add providers.provider_type (String(20), server_default="custom")
- Change provider_api_keys.api_key from VARCHAR(500) to TEXT (OAuth tokens can be long)

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


def column_exists(table_name: str, column_name: str) -> bool:
    """检查列是否已存在"""
    bind = op.get_bind()
    inspector = inspect(bind)
    columns = [col["name"] for col in inspector.get_columns(table_name)]
    return column_name in columns


def upgrade() -> None:
    # Add providers.provider_type
    if not column_exists("providers", "provider_type"):
        op.add_column(
            "providers",
            sa.Column("provider_type", sa.String(20), nullable=False, server_default="custom"),
        )

    # Expand provider_api_keys.api_key from VARCHAR(500) to TEXT
    op.alter_column(
        "provider_api_keys",
        "api_key",
        type_=sa.Text(),
        existing_type=sa.String(500),
        existing_nullable=False,
    )


def downgrade() -> None:
    # Revert provider_api_keys.api_key from TEXT to VARCHAR(500)
    # WARNING: Downgrade may fail if any api_key values exceed 500 characters
    op.alter_column(
        "provider_api_keys",
        "api_key",
        type_=sa.String(500),
        existing_type=sa.Text(),
        existing_nullable=False,
    )

    # Drop providers.provider_type
    if column_exists("providers", "provider_type"):
        op.drop_column("providers", "provider_type")
