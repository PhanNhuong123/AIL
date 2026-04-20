.PHONY: install-agents install-agents-dev install-agents-all test clean help
.DEFAULT_GOAL := help

help:
	@echo "Targets:"
	@echo "  install-agents       - pip install . (production)"
	@echo "  install-agents-dev   - pip install -e .[dev]"
	@echo "  install-agents-all   - pip install -e .[all,dev] (all provider SDKs)"
	@echo "  test                 - pytest tests/"
	@echo "  clean                - remove build/, dist/, *.egg-info, __pycache__"

install-agents:
	pip install .

install-agents-dev:
	pip install -e ".[dev]"

install-agents-all:
	pip install -e ".[all,dev]"

test:
	pytest tests/

clean:
	rm -rf build dist *.egg-info .pytest_cache
	find . -name __pycache__ -type d -exec rm -rf {} +
