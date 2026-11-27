.PHONY: build release run clean

# 默认目标：构建 release 版 exe
all: release

# Debug 构建
build:
	cargo build

# Release 构建（优化后的 exe）
release:
	cargo build --release

# 运行（debug 模式）
run:
	cargo run

# 运行（release 模式）
run-release:
	cargo run --release

# 清理构建产物
clean:
	cargo clean

# 显示构建产物路径
where:
	@echo "Debug:   target\debug\rdpmanager.exe"
	@echo "Release: target\release\rdpmanager.exe"
