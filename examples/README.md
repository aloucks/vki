# Examples

* [triangle](#trianglers)
* [triangle_multisample](#triangle_multisamplers)
* [cube](#cubers)
* [cube_texture](#cube_texturers)
* [particle_simulator](#particle_simulatorrs)
* [gltf_viewer](#gltf_viewerrs)

The triangle examples demonstrate initialization and basic event
handling, where as the remaining examples use a common framework to
manage these tasks. The examples framework has the following camera
controls:

### Camera Controls

|Key|Movement| 
|---|---| 
|`W`, `S`, `A`, `D`|Forward, Backward, Strafe-Left, Strafe-Right|
|`PageUp`, `PageDown`|Up, Down; (Hold `Shift` to preserve the focus point)|
|`C`|Set the camera's focus point at the origin|
|`F11`|Toggle fullscreen|
|`Mouse-Click-Drag`|Rotate the camera around the focus point|
|`Mouse-Scroll`|Move the camera toward or away from the focus point|

## triangle.rs

![triangle.rs](screenshots/triangle.png)


## triangle_multisample.rs

![triangle_multisample.rs](screenshots/triangle_multisample.png)


## cube.rs

![cube.rs](screenshots/cube.png)


## cube_texture.rs

![cube_texture.rs](screenshots/cube_texture.png)

## particle_simulator.rs

### Additional Controls

|Key|Action| 
|---|---| 
|`F2`|Reset position and velocity values|
|`F3`|Reset position values|

![particle_simulator.rs](screenshots/particle_simulator.png)

## gltf_viewer.rs

Sample models can be found here:

https://github.com/KhronosGroup/glTF-Sample-Models/tree/master/2.0

### Usage

```
cargo run --example gltf_viewer <FILE>
```

### Features

- [ ] Image Based Lighting (IBL)
- [X] Physically Based Rendering
- [X] Animation
- [X] Morph Targets (maximum of 2)


### Additional Controls

|Key|Action| 
|---|---| 
|`1` - `9`|Toggle animation channel|

### Examples

Note that the examples will look rather dark until IBL is implemented.

#### [BrainStem.gltf](https://github.com/KhronosGroup/glTF-Sample-Models/tree/master/2.0/BrainStem)

<img src="screenshots/gltf_viewer-BrainStem.gif" width="800"/>

#### [FlightHelment.gltf](https://github.com/KhronosGroup/glTF-Sample-Models/tree/master/2.0/FlightHelmet)

![gltf_viewer.rs](screenshots/gltf_viewer-FlightHelmet.png)

#### [DamagedHelmet.gltf](https://github.com/KhronosGroup/glTF-Sample-Models/tree/master/2.0/DamagedHelmet)

![gltf_viewer.rs](screenshots/gltf_viewer-DamagedHelmet.png)
