# 从源码构建：docker build -t congtodo .
# 发布流水线使用 Dockerfile.release（直接复制 CI 编译好的二进制）

# ---- 前端 ----
FROM node:22-alpine AS web
WORKDIR /app/web
COPY web/package*.json ./
RUN npm ci
COPY web/ ./
RUN npm run build

# ---- 后端（前端产物会被 rust-embed 打进二进制）----
FROM rust:1-bookworm AS server
WORKDIR /app
COPY server/ server/
COPY --from=web /app/web/dist web/dist
RUN cd server && cargo build --release --locked && mkdir -p /out/data

# ---- 运行时：distroless 自带 tzdata 和 CA 证书，无 shell，以非 root 用户运行 ----
FROM gcr.io/distroless/cc-debian12:nonroot
COPY --from=server --chown=65532:65532 /out/data /data
COPY --from=server /app/server/target/release/congtodo /congtodo
ENV CONGTODO_DB=/data/congtodo.db \
    CONGTODO_ADDR=0.0.0.0:8080 \
    TZ=Asia/Shanghai
VOLUME /data
EXPOSE 8080
ENTRYPOINT ["/congtodo"]
