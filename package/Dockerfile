# SPDX-License-Identifier: GPL-3.0-or-later
FROM ubuntu:20.04
ARG DEBIAN_FRONTEND=noninteractive
ARG pylon_VERSION=5.2.0.13457

RUN apt-get update
RUN apt-get -qq install cmake curl ninja-build ffmpeg libopencv-dev rsync clang libclang-dev unzip libglib2.0-dev libgtk-3-dev

# pylon (Basler cameras)
RUN cd /tmp && \
    curl -sSLO https://www2.baslerweb.com/fp-1551786516/media/downloads/software/pylon_software/pylon-${pylon_VERSION}-x86_64.tar.gz && \
    tar -xf pylon-${pylon_VERSION}-x86_64.tar.gz && \
    cd pylon-${pylon_VERSION}-x86_64 && \
    tar -xf pylonSDK-${pylon_VERSION}-x86_64.tar.gz && \
    cp -r pylon5 /opt && \
    cd .. && \
    rm -rf pylon-${pylon_VERSION}*

# rustup / cargo
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"
RUN rustup install 1.75 && \
    rustup default 1.75

# linuxdeploy
RUN curl -sSLO https://github.com/linuxdeploy/linuxdeploy/releases/download/1-alpha-20220822-1/linuxdeploy-x86_64.AppImage && \
    chmod +x linuxdeploy-x86_64.AppImage && \
    ./linuxdeploy-x86_64.AppImage --appimage-extract && \
    mv squashfs-root /opt/linuxdeploy && \
    rm linuxdeploy-x86_64.AppImage

# protobuf
RUN PB_REL="https://github.com/protocolbuffers/protobuf/releases" && \
    curl -sSLO $PB_REL/download/v3.15.8/protoc-3.15.8-linux-x86_64.zip && \
    unzip protoc-3.15.8-linux-x86_64.zip -d opt/protoc
