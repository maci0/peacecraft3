FROM debian:trixie-slim@sha256:a29215f6a35e51e22adffa17f89e9d2ef06214e64a2bad10d765c46aea49f11f
RUN dpkg --add-architecture i386 \
 && apt-get update \
 && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends wine wine32:i386 wine64 xvfb xauth xdotool x11-utils imagemagick mesa-utils libgl1:i386 libglx-mesa0:i386 libgl1-mesa-dri:i386 libgl1-mesa-dri libasound2-plugins:i386 \
 && rm -rf /var/lib/apt/lists/*
ENV HOME=/run/reference/home WINEPREFIX=/run/reference/prefix DISPLAY=:99 LIBGL_ALWAYS_SOFTWARE=1 WINEDEBUG=-all WINEDLLOVERRIDES=winemenubuilder.exe=d
WORKDIR /run/reference
RUN apt-get update \
 && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends pulseaudio pulseaudio-utils gstreamer1.0-plugins-base:i386 gstreamer1.0-plugins-good:i386 gstreamer1.0-libav:i386 \
 && rm -rf /var/lib/apt/lists/*
ENV XDG_RUNTIME_DIR=/run/reference/audio
RUN apt-get update \
 && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends x11-xserver-utils \
 && rm -rf /var/lib/apt/lists/*
