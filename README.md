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

# Architecture

The engine binaries are split into 3 categories:
1. Cargo rust libraries
2. Root engine binary
3. Small Hot-reloadable libraries

For the hot-reloadable modules to be fast, we dynamically link them to the cargo libraries crate
instead of static linking.

The root binary/library holds the core runtime that holds all the hot-reloadable stuff together. 
Mainly threading/jobs code and the entity framework.

## The Runtime
The core idea of the runtime is automatic scheduling based on explicitly declared data dependencies.
The constraints of this system are as such:

The graph must be statically verifiable on every mutation, possibly requiring batch mutations.
Graph nodes definitions must declare what data they produce as structs with members made of known types, and declare 
what data they need as types or structs of known types.

The graph nodes are bundled into defined entities. These might be called "actors", "clusters", "modules".
A defined entity of nodes represents an in-game concept sharing one lifetime. If the entity is destroyed, all of its nodes are also
disposed of.

A graph node may only be destroyed if there are no nodes that require its data. It may be destroyed if the dependents have a defined
strategy for decoupling from said deleted dependencies.

Graph nodes may be decoupled by using "queues". The idea is that a node may define a queue, and other nodes may produce data for that 
queue, the scheduler treats this as a hard dependency, and will require that the all operations that emit data to that queue are 
evaluated before that queue is consumed every frame.

All structs exported or consumed by nodes must have attached metadata about the naming, addresses, and types of their fields.

When a hot-reload occurs, every single cached struct in the graph is compared to its new definition and if possible is converted over,
all nodes inputs and outputs are compared to their new definitions, and if they're compatible with the new definitions, they'll be
converted over. In the event of a failure execution will stop. In the future we might be able to allow for "patches" where when 
hot reloading in a debug build the programmer will get a popup when there are conflicts that can't be automatically handled, and
be given a chance to try to fix those. If patches are ever applied to a live build of the game, explicit patches are a must.

# Binaries

Brane Weaver: Build/baking tool. Used for managing hot-reloading as well as compiling assets like
shaders ahead of time.

Brane Editor: Brane's version of the hammer editor, does not run the game at all but is used to 
edit chunks as well as launch the engine itself to test out said edits.

Brane Client: The game.

Brane Server: The server for the game.
