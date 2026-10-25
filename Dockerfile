# Immagine di solo runtime: il binario si compila prima con ./build.sh release
# (container con limite di RAM, DESIGN §1), qui si copia soltanto.
FROM gcr.io/distroless/cc-debian13:nonroot
COPY target/release/pumpbot /usr/local/bin/pumpbot
WORKDIR /data
ENTRYPOINT ["/usr/local/bin/pumpbot"]
CMD ["run", "--config", "/config/pumpbot.toml"]
