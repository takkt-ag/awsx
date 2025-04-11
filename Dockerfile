# Copyright 2025 TAKKT Industrial & Packaging GmbH
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.
#
# SPDX-License-Identifier: Apache-2.0

FROM rust:latest AS builder

ARG RUST_ARCH="x86_64-unknown-linux-musl"
ENV CC_x86_64_unknown_linux_musl=clang \
    RUST_BACKTRACE=full

RUN set -ex ;\
    export DEBIAN_FRONTEND=noninteractive ;\
    apt-get update ;\
    apt-get install -y --no-install-recommends \
      clang \
      ;\
    apt-get clean ;\
    rm -rf /var/lib/apt/lists/* ;\
    rustup target add "${RUST_ARCH}" ;\
    :

COPY . /app
WORKDIR /app

RUN set -ex ;\
    cargo build --target "${RUST_ARCH}" --release ;\
    cargo test --target "${RUST_ARCH}" --release -- --nocapture ;\
    mv target/"${RUST_ARCH}"/release/awsx awsx ;\
    :

FROM scratch

COPY --from=builder /app/awsx /awsx
ENTRYPOINT ["/awsx"]
CMD ["--help"]
