FROM scratch

WORKDIR /app

COPY target/release/rsqueue /app/rsqueue

ENTRYPOINT ["/app/rsqueue"]