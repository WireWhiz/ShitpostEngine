# This is my personal experiments engine

The goal of this project is to strike a balance between shipping things quickly, and letting me implement things from scratch.

I chose rust because it has a lot of quality of life features and libraries, which we're going to use, I'm going to have a hard rule of: 
"what the user sees, I made"

This includes any api that interacts with hardware other than raw network sockets, because I don't want to re-implement websockets or Quic fun.
* Graphics (Vulkan)
* UI (ImGui)
* Sound Output (Kernel APIs, we'll use SteamAudio for spatializtion)
* Physics (Jolt)
* VR (OpenXR)
* Windowing/Keyboard/Mouse input (Kernel APIs)

What this means is that in a lot of places I'm going to explicitly be using Rust as C, instead of as rust. Unsafe go brr.

I also want very robust fast hot-reload system. This is very hard to achieve with standard cargo or existing systems, so I'm creating a build system
that'll manually call rustc as needed to recompile individual binaries, as well as provide macros for easily managing the dynamically loaded libraries
we'll be hot-relading. This system can also be expanded to provide updates for other asset types such as compiling slang binaries and sending updates 
over the network if needed, like to quest.

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


