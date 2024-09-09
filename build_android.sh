#!/bin/sh
NAME_LIB='wgpu_game'
COLOR='\033[1;32m'

ANDROID_PROJECT=/home/gihexdev00/AndroidStudioProjects/WgpuGame
SDL_LIB=/home/gihexdev00/Game-Dev/lib/SDL-android

JNI_LIBS=${ANDROID_PROJECT}/app/src/main/jniLibs

TARGET=("aarch64-linux-android" "armv7-linux-androideabi" "i686-linux-android" "x86_64-linux-android")
ABI=("arm64-v8a" "armeabi-v7a" "x86" "x86_64")

echo -e "${COLOR}Cleaning output directory..."
for B in ${ABI[@]}; do
rm -f $JNI_LIBS/$B/lib${NAME_LIB}.so
done

echo -e "${COLOR}Checking output directory:"
for B in ${ABI[@]}; do
if [ ! -d $JNI_LIBS/$B ]; then
    echo -e "${COLOR}Directory\033[0m $JNI_LIBS/$B ${COLOR}not exist. Creating...\033[0m "
    mkdir $JNI_LIBS/$B
else
    echo -e "${COLOR}  Directory\033[0m $B ${COLOR}already exist.\033[0m"
fi
done

if [ $# -lt 1 ]; then
    build='debug'
else
    build=release
fi

echo -e "${COLOR}\nCopy SDl Lib to target ${build}:\033[0m"
for i in ${!TARGET[@]}; do
src=${SDL_LIB}/${build}/${ABI[$i]}/libSDL2.so
dest=./target/${TARGET[$i]}/${build}/deps/libSDL2.so
echo -e "${COLOR}  Copy SDl library:\033[0m $src ${COLOR}=>\033[0m $dest"
cp $src $dest
done

echo -e "${COLOR}\nBuild ${build}:\033[0m"
for i in ${!TARGET[@]}; do
echo -e "${COLOR}  Building ${build}\033[0m [${TARGET[$i]}]${COLOR}:\033[0m"
cargo build --target ${TARGET[$i]} --$1
done

echo -e "${COLOR}\nAdd library to Android Project:\033[0m"
for i in ${!TARGET[@]}; do
src=./target/${TARGET[$i]}/${build}/lib${NAME_LIB}.so
dest=$JNI_LIBS/${ABI[$i]}/lib${NAME_LIB}.so
echo -e "${COLOR}  Add library:\033[0m $src ${COLOR}=>\033[0m $dest"
cp $src $dest
done
