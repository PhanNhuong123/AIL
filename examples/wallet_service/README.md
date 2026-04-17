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

All commands run from inside the example directory.

```bash
cd examples/wallet_service

# 1. Migrate the filesystem source to SQLite (roundtrip-verified)
ail migrate --from src/ --to project.ail.db --verify

# 2. Verify (auto-detects project.ail.db via [database] backend = "auto")
ail verify

# 3. Build Python from SQLite
ail build --target python
pytest dist/tests/ -v

# 4. Build TypeScript from SQLite
ail build --target typescript
cd dist-ts
npm install
npx tsc --noEmit
npx vitest run
cd ..

# 5. Search via BM25 (FTS5) from SQLite
ail search "balance transfer"

# 6. Print a CIC context packet; second call hits cic_cache
ail context --task "add rate limiting"
ail context --task "add rate limiting"   # cache hit (cache_hit=true)
```

See `docs/plan/v2.0/plan/phases/12-integration-release.md` for the phase plan.
