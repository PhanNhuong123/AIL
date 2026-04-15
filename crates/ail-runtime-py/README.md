# ail-runtime

Runtime library for AIL-generated Python code.

Provides contract helpers (`pre`, `post`, `keep`), builtin type validators, and repository base classes used by code emitted by the AIL compiler.

## Install

```bash
pip install ail-runtime
```

## Usage

```python
from ail_runtime import pre, post, keep, ContractViolation
from ail_runtime import PositiveInteger, NonEmptyText
from ail_runtime import AilRepository, AsyncAilRepository
```
