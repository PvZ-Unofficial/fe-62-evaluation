# PvZ-Emulator
Plants vs. Zombies Survival Endless emulator.

This emulator can emulate all kinds of plants and zombies except Dr. Zomboss
and Zombie Bobsled Team (They don't occur in Survival Endless). It supports
all 6 scenes (Day, Night, Pool, Fog, Roof, and Moon Night).

### Install
To build the Python module with CMake, install pybind11 3.x and add
`-DPVZEMU_BUILD_PYBIND=True` to your CMake build command.

You can install through `pip` as well:
```shell script
python -m pip install git+https://github.com/dnartz/PvZ-Emulator
```

## C++ editor and static analysis

Install LLVM (clangd, clang-tidy and clang-cl) and Ninja. On Windows, run the
commands below in an x64 Visual Studio developer shell and append
`-DCMAKE_CXX_COMPILER=clang-cl` to the configure command. This uses the MSVC
headers and SDK while avoiding localized MSVC include-output encoding issues.
Generate the compilation database used by `.clangd`:

```sh
cmake -S . -B out/build/clangd -G Ninja -DCMAKE_BUILD_TYPE=Debug -DCMAKE_EXPORT_COMPILE_COMMANDS=ON -DPVZEMU_BUILD_TESTS=ON -DPVZEMU_BUILD_PYBIND=OFF -DPVZEMU_BUILD_DEBUGGER=OFF
cmake --build out/build/clangd
ctest --test-dir out/build/clangd --output-on-failure
clangd --check=world.cpp --check-lines=1-10
clang-tidy -p out/build/clangd world.cpp
```

Repeat the configure command when sources or build options change. Keep the
generated database in the build directory; it contains machine-specific paths.
For another build directory, override `CompileFlags.CompilationDatabase` in
your personal clangd configuration. GCC users may also need to allow their
compiler with clangd's `--query-driver` editor argument.

No project `.clang-tidy` configuration is provided. clangd uses its default
embedded checks except `bugprone-integer-division`, disabled in `.clangd`. These
diagnostics are advisory: review findings against the emulator's intended
behavior before changing code, and do not apply bulk fixes.

## Usage
This emulator can be used as a static library or a python module. Include
`pvz-emulator/world.h` before to your source code. A world instance must stay
on one thread; separate threads may own separate world instances.

### Create and Update a Game Instance
```cpp
int main(void) {
    pvz_emulator::world w(pvz_emulator::object::scene_type::pool);
    w.reset();

    // The first wave of zombies will spawn after 6 seconds.
    for (int i = 0; i < 600; i++) {
        w.update();
    }

    return 0;
}
```

```python
from pvzemu import World, SceneType

w = World(SceneType.pool)
w.reset()
for i in range(0, 600):
    w.update()
```

### Control Flags
You can set some bool flags in the `scene` objects to enable or disable some
easter eggs and bugs that affect motions of objects.
* `is_zombie_dance` Enable the "dance" effect if set to `true`.
* `is_future_enalbed` Enable the "future" effect if set to `true`.
* `stop_spawn` Stop spawning zombies if set to `true`.
* `enable_split_pea_bug` Enable the split pea shooting-state bug if set to `true`.

## Debugging
I wrote a simple web debugger to visualize the game state. To compile the
debugger, you need to [install node.js](https://nodejs.org/en/download/) and
[extract](https://plantsvszombies.fandom.com/wiki/Modify_Plants_vs._Zombies)
some images from `main.pak` in the game installation directory.

After unpacked `main.pak`, copy these files to `debugger/src/assets`:
```console
$ cd images
$ cp background1.jpg PATH_TO_THE_SOURCE_CODE/debugger/src/assets/day.jpg
$ cp background2.jpg PATH_TO_THE_SOURCE_CODE/debugger/src/assets/night.jpg
$ cp background3.jpg PATH_TO_THE_SOURCE_CODE/debugger/src/assets/pool.jpg
$ cp background4.jpg PATH_TO_THE_SOURCE_CODE/debugger/src/assets/fog.jpg
$ cp background5.jpg PATH_TO_THE_SOURCE_CODE/debugger/src/assets/roof.jpg
$ cp background6.jpg PATH_TO_THE_SOURCE_CODE/debugger/src/assets/moon_night.jpg
```

Then you can build the debug server:
```console
$ cmake PATH_TO_THE_SOURCE_CODE -DPVZEMU_BUILD_DEBUGGER=True
$ make
```

Run the debug server:
```console
$ ./debug-server PATH_TO_THE_SOURCE_CODE/debugger/dist
```

Now you can Open http://localhost:3000 on your browser.
