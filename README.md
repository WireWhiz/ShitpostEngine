# This is my personal experiments engine

The goal of this project is to strike a ballance between shipping things quickly, and letting me implement things from scratch.

I chose rust because it has a lot of quality of life features and libraries, which we're going to use, I'm going to have a hard rule of: 
"what the user sees, I made"

This includes any api that interacts with hardware other than raw network sockets, because I don't want to re-implement websockets for fun.
* Graphics (Vulkan)
* Sound Output (Kernel APIs, we'll use SteamAudio for spatializtion)
* VR (OpenXR)
* Windowing/Keyboard/Mouse input (Kernel APIs)

What this means is that in a lot of places I'm going to explicitly be using Rust as C, instead of as rust. Unsafe go brr.

# Conventions (since this is important)

Coordinates:
The GLTF standard
x-right
y-up
z-forward
Right-handed rotations
1 is a meter 

Matrix:
Column major


