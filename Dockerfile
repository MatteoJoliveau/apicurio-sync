FROM lukemathwalker/cargo-chef as planner

WORKDIR app
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM lukemathwalker/cargo-chef as cacher

WORKDIR app
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

FROM rust:1 as build

WORKDIR /app
COPY . .
COPY --from=cacher /app/target target
COPY --from=cacher $CARGO_HOME $CARGO_HOME
RUN ls
RUN cargo build --release

FROM public.ecr.aws/bitnami/minideb:buster
COPY --from=build /app/target/release/apicurio-sync /usr/local/bin/
ENTRYPOINT ["/usr/local/bin/apicurio-sync"]
