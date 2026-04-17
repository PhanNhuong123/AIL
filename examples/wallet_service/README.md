# wallet_service

Canonical v2.0 end-to-end example for AIL.

## Layout

```
wallet_service/
├── ail.config.toml          # project config, database backend = "auto"
├── src/                     # AIL source (flat; proven through the pipeline)
│   ├── wallet_balance.ail
│   ├── positive_amount.ail
│   ├── user.ail
│   ├── transfer_result.ail
│   ├── add_money.ail
│   ├── deduct_money.ail
│   └── transfer_money.ail
├── project.ail.db           # (generated) SQLite backend after `ail migrate`
├── dist/                    # (generated) Python output after `ail build --target python`
└── dist-ts/                 # (generated) TypeScript output after `ail build --target typescript`
```

## v2.0 Pipeline

```bash
# 1. Migrate the filesystem source to SQLite
ail migrate --from examples/wallet_service/src/ --to examples/wallet_service/project.ail.db --verify

# 2. Verify (auto-detects .ail.db when present)
ail verify --project examples/wallet_service/

# 3. Build Python from SQLite (byte-identical to filesystem build)
ail build --target python --project examples/wallet_service/

# 4. Run the generated tests
pytest examples/wallet_service/dist/tests/ -v

# 5. Search via BM25 (FTS5) from SQLite
ail search "balance transfer" --project examples/wallet_service/

# 6. Check CIC context-packet cache hits after warmup
ail context --task "add rate limiting" --project examples/wallet_service/
ail context --task "add rate limiting" --project examples/wallet_service/  # cache hit
```

See `docs/plan/v2.0/plan/phases/12-integration-release.md` for the phase plan.
