# Wiki AIL

Đây là wiki tiếng Việt cho repo **AIL - AI Layer**. Mục tiêu của bộ tài liệu này là giúp người mới nắm được ý tưởng sản phẩm, kiến trúc codebase, các crate Rust, CLI/MCP, Python agent, cấu hình và cách vận hành/test.

## Mục lục đề xuất

1. [Tổng quan repo](./01-tong-quan.md)
2. [Rust core và pipeline kiểm chứng](./02-rust-core.md)
3. [CLI, MCP và workflow người dùng](./03-cli-mcp-workflows.md)
4. [Python agent và runtime Python](./04-python-agent.md)
5. [Testing, cấu hình và vận hành](./05-testing-config-operations.md)
6. [IDE, UI và Tauri bridge](./06-ide-ui.md)

## Repo này giải quyết việc gì?

AIL cho phép mô tả hệ thống bằng các file `.ail` theo dạng tiếng Anh có cấu trúc. Toolchain biến mô tả đó thành graph, lan truyền ràng buộc qua CIC, kiểm tra bằng type system và Z3, rồi phát sinh code Python/TypeScript cùng test/source map. Thay vì để constraint nằm trong trí nhớ của AI hoặc comment rời rạc, AIL đưa constraint vào cấu trúc graph để có thể truy vết và kiểm chứng lại.

## Luồng đọc nhanh

- Muốn hiểu repo ở mức sản phẩm: đọc `01-tong-quan.md`.
- Muốn sửa Rust core: đọc `02-rust-core.md` trước, sau đó mở crate liên quan.
- Muốn dùng tool: đọc `03-cli-mcp-workflows.md`.
- Muốn chạy hoặc phát triển agent: đọc `04-python-agent.md`.
- Muốn chạy test, migration, coverage hoặc kiểm tra cấu hình: đọc `05-testing-config-operations.md`.
- Muốn làm visual IDE/Tauri/Svelte: đọc `06-ide-ui.md`.

## Nguồn tham khảo chính

- `README.md`
- `GETTING_STARTED.md`
- `docs/config-reference.md`
- `docs/README.md`
- `agents/README.md`
- `crates/*/src` và `crates/*/tests`
- `examples/wallet_service`
- `ide/` và `crates/ail-ui-bridge`
