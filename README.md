# protostar
A collection of application launchers for Stardust XR
> [!IMPORTANT]  
> Requires the [Stardust XR Server](https://github.com/StardustXR/server) to be running. For launching 2D applications, [Flatland](https://github.com/StardustXR/flatland) also needs to be running.  

If you installed the Stardust XR server via:  
```note
sudo dnf group install stardust-xr
```
Or if you installed via the [installation script](https://github.com/cyberneticmelon/usefulscripts/blob/main/stardustxr_setup.sh), Protostar comes pre-installed

# How to Use
Protostar itself can be used to build various kinds of app launchers, but two are built in. Most likely the one you will want to use will be `hexagon_launcher`. After launching, in flastcreen mode drag applications out of the app launcher, hold down `Shift + ~`
![updated_drag](https://github.com/StardustXR/website/blob/main/static/img/updated_flat_drag.GIF)
**Quest 3 Hand tracking**:
Pinch to drag and drop, grasp with full hand for grabbing, point and click with pointer finger to click or pinch from a distance  

![hand_pinching](https://github.com/StardustXR/website/blob/main/static/img/hand_pinching.GIF)

## Manual Installation
Clone the repository and after the server is running:
```sh
cargo run
```
