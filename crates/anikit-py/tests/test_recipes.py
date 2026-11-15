"""Run every ```python block of docs/recipes.md, so the cookbook stays correct."""

import pathlib
import re

import pytest

import anikit

RECIPES = pathlib.Path(__file__).resolve().parents[3] / "docs" / "recipes.md"
BLOCKS = re.findall(r"^```python\n(.*?)^```", RECIPES.read_text(encoding="utf-8"), re.S | re.M) if RECIPES.exists() else []


@pytest.mark.skipif(not BLOCKS, reason="docs/recipes.md not found (installed package)")
@pytest.mark.parametrize("code", BLOCKS, ids=[f"block{i}" for i in range(len(BLOCKS))])
def test_recipe_block_runs(code, capsys):
    anikit.kill_all()
    exec(compile(code, str(RECIPES), "exec"), {"__name__": "__recipe__"})
    anikit.kill_all()
