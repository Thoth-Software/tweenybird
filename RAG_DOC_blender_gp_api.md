# Blender Python API Reference for Tweenybird (Grease Pencil Addon)

> Extracted from the Blender 5.0 Python API documentation.
> This file covers every API surface used by the Tweenybird addon and related
> APIs likely needed for extending it.

---

## Table of Contents

1. [Addon Structure (Registration)](#1-addon-structure-registration)
2. [bpy.props -- Property Definitions](#2-bpyprops----property-definitions)
3. [bpy.types.Operator](#3-bpytypesoperator)
4. [bpy.types.Panel](#4-bpytypespanel)
5. [bpy.types.AddonPreferences](#5-bpytypesaddonpreferences)
6. [bpy.types.PropertyGroup](#6-bpytypespropertygroup)
7. [bpy.types.UILayout](#7-bpytypesuilayout)
8. [bpy.types.WindowManager](#8-bpytypeswindowmanager)
9. [Grease Pencil Data Model](#9-grease-pencil-data-model)
    - [GreasePencil (ID)](#greasepencil-id)
    - [GreasePencilv3Layers](#greasepencilv3layers)
    - [GreasePencilLayer](#greasepencillayer)
    - [GreasePencilFrame](#greasepencilframe)
    - [GreasePencilFrames](#greasepencilframes)
    - [GreasePencilDrawing](#greasepencildrawing)
10. [bpy.ops.grease_pencil -- Grease Pencil Operators](#10-bpyopsgrease_pencil)
11. [bpy.ops.render](#11-bpyopsrender)
12. [bpy.ops.object](#12-bpyopsobject)
13. [bpy.ops.preferences](#13-bpyopspreferences)
14. [bpy.types.Object](#14-bpytypesobject)
15. [bpy.types.Scene](#15-bpytypesscene)
16. [bpy.types.RenderSettings & ImageFormatSettings](#16-rendersettings--imageformatsettings)
17. [bpy.context](#17-bpycontext)
18. [bpy.path](#18-bpypath)
19. [bpy.data](#19-bpydata)
20. [bpy.utils](#20-bpyutils)

---

## 1. Addon Structure (Registration)

### bl_info dict

Required metadata for legacy addons (Blender < 4.2 style). For Blender 4.2+ extensions,
use `blender_manifest.toml` instead.

```python
bl_info = {
    "name": "GP AI Inbetween",
    "author": "...",
    "version": (0, 1, 0),
    "blender": (4, 0, 0),
    "location": "View3D > Sidebar > GP AI",
    "description": "...",
    "warning": "...",
    "doc_url": "...",
    "category": "Animation",
}
```

### register / unregister

```python
def register():
    for cls in classes:
        bpy.utils.register_class(cls)
    # Attach custom properties to existing types
    bpy.types.Scene.my_props = bpy.props.PointerProperty(type=MyPropertyGroup)

def unregister():
    del bpy.types.Scene.my_props
    for cls in reversed(classes):
        bpy.utils.unregister_class(cls)
```

---

## 2. bpy.props -- Property Definitions

This module defines properties to extend Blender's internal data. All parameters must
be passed as keywords. Properties can be animated, accessed from UI and Python.

### BoolProperty

```python
bpy.props.BoolProperty(
    *, name='', description='', default=False,
    options={'ANIMATABLE'}, override=set(), tags=set(),
    subtype='NONE', update=None, get=None, set=None
)
```

### IntProperty

```python
bpy.props.IntProperty(
    *, name='', description='', default=0,
    min=-2**31, max=2**31-1, soft_min=-2**31, soft_max=2**31-1,
    step=1, options={'ANIMATABLE'}, override=set(), tags=set(),
    subtype='NONE', update=None, get=None, set=None
)
```

### FloatProperty

```python
bpy.props.FloatProperty(
    *, name='', description='', default=0.0,
    min=sys.float_info.min, max=sys.float_info.max,
    soft_min=-inf, soft_max=inf,
    step=3, precision=2,
    options={'ANIMATABLE'}, override=set(), tags=set(),
    subtype='NONE', unit='NONE', update=None, get=None, set=None
)
```

### StringProperty

```python
bpy.props.StringProperty(
    *, name='', description='', default='', maxlen=0,
    options={'ANIMATABLE'}, override=set(), tags=set(),
    subtype='NONE',  # subtypes: 'FILE_PATH', 'DIR_PATH', 'FILE_NAME', 'PASSWORD', 'BYTE_STRING', 'NONE'
    update=None, get=None, set=None, search=None, search_options={'SUGGESTION'}
)
```

### EnumProperty

```python
bpy.props.EnumProperty(
    *, name='', description='',
    items,  # list of (identifier, name, description[, icon[, number]]) or callback
    default=None,
    options={'ANIMATABLE'}, override=set(), tags=set(),
    update=None, get=None, set=None
)
```

**items** can be a static list or a callback `function(self, context) -> list`.
Each item is a tuple: `(identifier, name, description)` or
`(identifier, name, description, icon, number)`.

### PointerProperty

```python
bpy.props.PointerProperty(
    *, name='', description='',
    type=None,  # A PropertyGroup subclass
    options={'ANIMATABLE'}, override=set(), tags=set(),
    update=None, poll=None
)
```

Used to attach a PropertyGroup to an existing Blender type:
```python
bpy.types.Scene.my_settings = bpy.props.PointerProperty(type=MyPropertyGroup)
```

### CollectionProperty

```python
bpy.props.CollectionProperty(
    *, name='', description='',
    type=None,  # A PropertyGroup subclass
    options={'ANIMATABLE'}, override=set(), tags=set()
)
```

### Update Callbacks

All properties except CollectionProperty support `update` callbacks:
```python
def update_func(self, context):
    print("property changed", self)

bpy.types.Scene.my_prop = bpy.props.FloatProperty(update=update_func)
```

**Warning:** Update callbacks may execute in threaded context.

---

## 3. bpy.types.Operator

Base class: `bpy_struct`

Storage of an operator being executed, or registered after execution.

### Class Attributes

| Attribute | Type | Description |
|-----------|------|-------------|
| `bl_idname` | str | Unique operator identifier (e.g. `"gpai.generate_inbetweens"`) |
| `bl_label` | str | Display name |
| `bl_description` | str | Tooltip text |
| `bl_options` | set | Options: `{'REGISTER', 'UNDO', 'BLOCKING', 'MACRO', 'INTERNAL', 'DEPENDS_ON_CURSOR'}` |

### Instance Attributes

| Attribute | Type | Description |
|-----------|------|-------------|
| `layout` | `UILayout` (readonly) | Layout for `draw()` |
| `has_reports` | bool (readonly) | Whether operator has reports |

### Methods

```python
@classmethod
def poll(cls, context) -> bool:
    """Test if the operator can be called."""

def invoke(self, context, event) -> set:
    """Called when operator is invoked (from UI/shortcut). Returns enum set in Operator Return Items."""

def execute(self, context) -> set:
    """Execute the operator. Returns {'FINISHED'} or {'CANCELLED'}."""

def modal(self, context, event) -> set:
    """Modal operator function. Returns {'RUNNING_MODAL'}, {'FINISHED'}, or {'CANCELLED'}."""

def draw(self, context):
    """Custom UI drawing for operator dialog."""

def cancel(self, context):
    """Called when operator is cancelled."""

def report(self, type, message):
    """Report a message. type is enum set: {'DEBUG', 'INFO', 'OPERATOR', 'WARNING', 'ERROR', 'ERROR_INVALID_INPUT'}"""

@classmethod
def poll_message_set(cls, message, *args):
    """Set tooltip message shown when poll() fails."""
```

### Operator Return Items
- `{'FINISHED'}` -- Operator completed successfully
- `{'CANCELLED'}` -- Operator was cancelled (no undo step)
- `{'RUNNING_MODAL'}` -- Operator is in modal mode
- `{'PASS_THROUGH'}` -- Pass the event through to other operators

### Undo Behavior

Any operator modifying Blender data **must** include `'UNDO'` in `bl_options`.
This creates an automatic undo step when the operator returns `{'FINISHED'}`.
When returning `{'CANCELLED'}`, no undo step is created.

### Dialog Example (used by Tweenybird)

```python
class MyOperator(bpy.types.Operator):
    bl_idname = "my.operator"
    bl_label = "My Operator"

    my_prop: bpy.props.IntProperty(name="Count", default=4)

    def invoke(self, context, event):
        return context.window_manager.invoke_props_dialog(self)

    def draw(self, context):
        self.layout.prop(self, "my_prop")

    def execute(self, context):
        # ... do work ...
        return {'FINISHED'}
```

---

## 4. bpy.types.Panel

Base class: `bpy_struct`

Panel containing UI elements.

### Class Attributes

| Attribute | Type | Description |
|-----------|------|-------------|
| `bl_idname` | str | Custom ID (defaults to class name) |
| `bl_label` | str | Panel header label |
| `bl_space_type` | enum | Space type: `'VIEW_3D'`, `'PROPERTIES'`, `'IMAGE_EDITOR'`, etc. |
| `bl_region_type` | enum | Region type: `'UI'` (sidebar), `'WINDOW'`, `'HEADER'`, `'TOOLS'`, etc. |
| `bl_category` | str | Tab name in sidebar (N-panel) |
| `bl_context` | str | Context filter (e.g. `"object"` for Properties editor) |
| `bl_options` | set | Options: `{'DEFAULT_CLOSED', 'HIDE_HEADER', 'INSTANCED', 'HEADER_LAYOUT_EXPAND'}` |
| `bl_parent_id` | str | Makes this a sub-panel of another panel |
| `bl_order` | int | Sort order (lower = first) |

### Instance Attributes

| Attribute | Type | Description |
|-----------|------|-------------|
| `layout` | `UILayout` (readonly) | The panel's layout |
| `is_popover` | bool (readonly) | Whether displayed as popover |

### Methods

```python
@classmethod
def poll(cls, context) -> bool:
    """Return True if panel should be drawn."""

def draw(self, context):
    """Draw UI elements into the panel layout."""

def draw_header(self, context):
    """Draw UI elements into the panel header."""
```

### Mix-in Pattern (used by Tweenybird)

```python
class GPAI_PT_MainPanel(bpy.types.Panel):
    bl_label = "GP AI Inbetween"
    bl_idname = "GPAI_PT_main_panel"
    bl_space_type = 'VIEW_3D'
    bl_region_type = 'UI'
    bl_category = 'GP AI'

    def draw(self, context):
        layout = self.layout
        obj = context.active_object
        if obj is None or obj.type != 'GREASEPENCIL':
            layout.label(text="Select a Grease Pencil object", icon='INFO')
            return
        layout.operator("gpai.generate_inbetweens", icon='RENDER_ANIMATION')
```

---

## 5. bpy.types.AddonPreferences

Base class: `bpy_struct`

Stores persistent addon preferences. Accessed via:
```python
prefs = context.preferences.addons[__package__].preferences
```

### Class Attributes

| Attribute | Type | Description |
|-----------|------|-------------|
| `bl_idname` | str | Must match `__name__` (or `__package__` for sub-module addons) |

### Usage Pattern

```python
class MyPreferences(bpy.types.AddonPreferences):
    bl_idname = __name__

    api_key: StringProperty(name="API Key", subtype='PASSWORD')
    threshold: FloatProperty(name="Threshold", default=0.85, min=0.0, max=1.0)

    def draw(self, context):
        layout = self.layout
        layout.prop(self, "api_key")
        layout.prop(self, "threshold", slider=True)
```

---

## 6. bpy.types.PropertyGroup

Base class: `bpy_struct`

Dynamically defined sets of properties. Can extend existing Blender data.
Must be registered before assigning to Blender types.

```python
class GPAI_SceneProperties(bpy.types.PropertyGroup):
    character_name: StringProperty(name="Character", default="")
    last_motion_type: StringProperty(name="Last Motion Type", default="")

# In register():
bpy.types.Scene.gpai = bpy.props.PointerProperty(type=GPAI_SceneProperties)

# In unregister():
del bpy.types.Scene.gpai

# Access:
context.scene.gpai.character_name
```

---

## 7. bpy.types.UILayout

Base class: `bpy_struct`

User interface layout in a panel or header.

### Properties

| Property | Type | Description |
|----------|------|-------------|
| `scale_x` | float | Scale factor along X |
| `scale_y` | float | Scale factor along Y |
| `active` | bool | Greyed out when False |
| `enabled` | bool | Disabled when False |
| `alert` | bool | Highlights the element |
| `alignment` | enum | `'EXPAND'`, `'LEFT'`, `'CENTER'`, `'RIGHT'` |
| `operator_context` | enum | Default: `'INVOKE_DEFAULT'` |

### Layout Methods

```python
# Sub-layouts
row(*, align=False, heading='') -> UILayout
column(*, align=False, heading='') -> UILayout
box() -> UILayout  # Items in a bordered box
split(*, factor=0.0, align=False) -> UILayout
grid_flow(*, row_major=False, columns=0, even_columns=False, even_rows=False, align=False) -> UILayout
column_flow(*, columns=0, align=False) -> UILayout

# Collapsible panel within layout
panel(idname, *, default_closed=False) -> (UILayout, UILayout)  # (header, body)
```

### Widget Methods

```python
# Property display
prop(data, property, *, text='', icon='NONE', expand=False, slider=False, toggle=-1, icon_only=False, index=-1)

# Operator button
operator(operator, *, text='', icon='NONE', emboss=True, depress=False, icon_value=0) -> OperatorProperties

# Static text label
label(*, text='', icon='NONE', icon_value=0)

# Visual separator
separator(*, factor=1.0)
separator_spacer()

# Menu
menu(menu, *, text='', icon='NONE')

# Enum property variations
prop_menu_enum(data, property, *, text='', icon='NONE')
props_enum(data, property)
prop_enum(data, property, value, *, text='', icon='NONE')
prop_search(data, property, search_data, search_property, *, text='', icon='NONE')

# Template methods (common UI patterns)
template_ID(data, property)
template_list(listtype_name, list_id, dataptr, propname, active_dataptr, active_propname)
```

### Common Icon Names

Used throughout: `'INFO'`, `'ERROR'`, `'PLAY'`, `'RENDER_ANIMATION'`, `'PREFERENCES'`,
`'CHECKMARK'`, `'X'`, `'GRAPH'`, `'TIME'`, `'WORLD'`, `'SETTINGS'`, `'NONE'`.

---

## 8. bpy.types.WindowManager

### invoke_props_dialog

```python
WindowManager.invoke_props_dialog(
    operator, *, width=300, title='', confirm_text='', cancel_default=False
) -> set  # Operator Return Items
```

Shows a dialog with operator properties. Calls `operator.draw()` for custom layout,
then `operator.execute()` when user clicks OK.

### popup_menu

```python
WindowManager.popup_menu(draw_func, *, title='', icon='NONE')
```

Display a popup menu. `draw_func` receives `(self, context)` where `self.layout` is
a `UILayout`. Does not block script execution.

```python
def draw_popup(self, context):
    for line in lines:
        self.layout.label(text=line)

context.window_manager.popup_menu(draw_popup, title="Stats", icon="INFO")
```

### modal_handler_add

```python
WindowManager.modal_handler_add(operator) -> bool
```

Register a modal handler for the given operator (call from `invoke()` before
returning `{'RUNNING_MODAL'}`).

### fileselect_add

```python
WindowManager.fileselect_add(operator)
```

Opens the file browser. Call from `invoke()`, return `{'RUNNING_MODAL'}`.

---

## 9. Grease Pencil Data Model

### Data Access Chain

```
context.active_object              -> Object (type='GREASEPENCIL')
    .data                          -> GreasePencil (ID)
        .layers                    -> GreasePencilv3Layers (collection of GreasePencilLayer)
            .active                -> GreasePencilLayer
                .frames            -> GreasePencilFrames (collection of GreasePencilFrame)
                    [i]            -> GreasePencilFrame
                        .drawing   -> GreasePencilDrawing
                        .frame_number -> int (readonly)
                        .select    -> bool
                        .keyframe_type -> enum
```

### GreasePencil (ID)

```python
class bpy.types.GreasePencil(ID)
```

Grease Pencil data-block. Base classes: `bpy_struct`, `ID`.

| Property | Type | Description |
|----------|------|-------------|
| `layers` | `GreasePencilv3Layers` collection of `GreasePencilLayer` (readonly) | All layers |
| `layer_groups` | `GreasePencilv3LayerGroup` collection (readonly) | Layer groups |
| `materials` | `IDMaterials` collection of `Material` (readonly) | Materials |
| `animation_data` | `AnimData` (readonly) | Animation data |
| `use_autolock_layers` | bool | Auto-lock all except active layer |
| `onion_mode` | enum `'ABSOLUTE'`/`'RELATIVE'`/`'SELECTED'` | Onion skinning mode |
| `onion_factor` | float [0,1] | Onion frame fade opacity |
| `onion_keyframe_type` | enum | Filter onion by keyframe type |
| `ghost_before_range` | int [0,120] | Frames to show before current |
| `ghost_after_range` | int [0,120] | Frames to show after current |
| `stroke_depth_order` | enum | Stroke ordering (`'2D'` default) |
| `use_onion_fade` | bool | Fade onion colors |
| `use_onion_loop` | bool | Loop onion display |

### GreasePencilv3Layers

```python
class bpy.types.GreasePencilv3Layers(bpy_struct)
```

Collection of Grease Pencil layers.

| Property | Type | Description |
|----------|------|-------------|
| `active` | `GreasePencilLayer` | The active (selected) layer |

```python
new(name, *, set_active=True, layer_group=None) -> GreasePencilLayer
    # name: str -- Layer name
    # set_active: bool -- Make new layer active
    # layer_group: GreasePencilLayerGroup or None -- Parent group

remove(layer)
    # layer: GreasePencilLayer -- Layer to remove

move(layer, type)
    # type: 'DOWN' or 'UP'

move_top(layer)
move_bottom(layer)
move_to_layer_group(layer, layer_group)
```

### GreasePencilLayer

```python
class bpy.types.GreasePencilLayer(GreasePencilTreeNode)
```

Collection of related drawings. Inherits from `GreasePencilTreeNode`.

| Property | Type | Description |
|----------|------|-------------|
| `frames` | `GreasePencilFrames` collection of `GreasePencilFrame` (readonly) | Keyframes |
| `blend_mode` | enum | `'REGULAR'`, `'HARDLIGHT'`, `'ADD'`, `'SUBTRACT'`, `'MULTIPLY'`, `'DIVIDE'` |
| `opacity` | float [0,1] | Layer opacity |
| `lock_frame` | bool | Lock displayed frame |
| `parent` | `Object` | Parent object |
| `parent_bone` | str | Parent bone name (for armature parent) |
| `pass_index` | int | Render pass index |
| `tint_color` | `Color` [0,1] | Tint color |
| `tint_factor` | float [0,1] | Tint amount |
| `translation` | `Vector` | Layer translation |
| `rotation` | `Euler` | Layer rotation |
| `scale` | `Vector` | Layer scale |
| `use_lights` | bool | Enable lighting |
| `matrix_local` | `Matrix` 4x4 (readonly) | Local transform matrix |

Inherited from `GreasePencilTreeNode`:
| Property | Type | Description |
|----------|------|-------------|
| `name` | str | Layer name |
| `hide` | bool | Visibility |
| `lock` | bool | Edit lock |
| `select` | bool | Selection state |
| `use_onion_skinning` | bool | Onion skin toggle |
| `channel_color` | `Color` | Channel display color |

```python
get_frame_at(frame_number) -> GreasePencilFrame
    # Returns the frame at the given frame number

current_frame() -> GreasePencilFrame
    # Returns the frame at the current scene time
```

### GreasePencilFrame

```python
class bpy.types.GreasePencilFrame(bpy_struct)
```

A Grease Pencil keyframe.

| Property | Type | Description |
|----------|------|-------------|
| `drawing` | `GreasePencilDrawing` | The drawing data |
| `frame_number` | int (readonly) | Frame number in scene |
| `select` | bool | Selection in Dope Sheet |
| `keyframe_type` | enum | `'KEYFRAME'`, `'BREAKDOWN'`, `'MOVING_HOLD'`, `'EXTREME'`, `'JITTER'`, `'GENERATED'` |

### GreasePencilFrames

```python
class bpy.types.GreasePencilFrames(bpy_struct)
```

Collection of Grease Pencil frames.

```python
new(frame_number) -> GreasePencilFrame
    # frame_number: int [-1048574, 1048574]

remove(frame_number)
    # frame_number: int [-1048574, 1048574]

copy(from_frame_number, to_frame_number, *, instance_drawing=False) -> GreasePencilFrame
    # instance_drawing: if True, copied frame shares the same drawing

move(from_frame_number, to_frame_number) -> GreasePencilFrame
```

### GreasePencilDrawing

```python
class bpy.types.GreasePencilDrawing(bpy_struct)
```

A Grease Pencil drawing.

| Property | Type | Description |
|----------|------|-------------|
| `attributes` | collection of `Attribute` (readonly) | Geometry attributes |
| `color_attributes` | collection of `Attribute` (readonly) | Color attributes |
| `curve_offsets` | collection of `IntAttributeValue` (readonly) | Curve start indices |
| `type` | enum (readonly) | `'DRAWING'` or `'REFERENCE'` |
| `user_count` | int (readonly) | Number of keyframes using this drawing |
| `strokes` | readonly collection | All strokes (not for performance-critical code; use `attributes` instead) |

```python
add_strokes(sizes)
    # sizes: int array -- Number of points per new stroke

remove_strokes(*, indices=(0,))
    # indices: optional int array -- Strokes to remove (all if not given)

resize_strokes(sizes, *, indices=(0,))
    # sizes: int array -- New point counts

reorder_strokes(new_indices)
    # new_indices: int array -- New ordering

tag_positions_changed()
    # Call after modifying point positions

vertex_group_assign(vgroup_name, indices_ptr, weight)
vertex_group_remove(vgroup_name, indices_ptr)
set_vertex_weights(vertex_group_name, indices, weights, *, assign_mode='REPLACE')
    # assign_mode: 'REPLACE', 'ADD', 'SUBTRACT'
```

---

## 10. bpy.ops.grease_pencil

### trace_image

```python
bpy.ops.grease_pencil.trace_image(
    *, target='NEW', radius=0.01, threshold=0.5,
    turnpolicy='MINORITY', mode='SINGLE',
    use_current_frame=True, frame_number=0
)
```

Extract Grease Pencil strokes from image.

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `target` | enum | `'NEW'` | `'NEW'` or `'SELECTED'` -- target GP object |
| `radius` | float [0.001, 1] | 0.01 | Stroke radius |
| `threshold` | float [0, 1] | 0.5 | Color threshold for stroke generation |
| `turnpolicy` | enum | `'MINORITY'` | `'FOREGROUND'`, `'BACKGROUND'`, `'LEFT'`, `'RIGHT'`, `'MINORITY'`, `'MAJORITY'`, `'RANDOM'` |
| `mode` | enum | `'SINGLE'` | `'SINGLE'` (current frame) or `'SEQUENCE'` (all frames) |
| `use_current_frame` | bool | True | Start at current frame |
| `frame_number` | int [0, 9999] | 0 | Specific frame to trace (0 = all) |

---

## 11. bpy.ops.render

### opengl

```python
bpy.ops.render.opengl(
    *, animation=False, render_keyed_only=False,
    sequencer=False, write_still=False, view_context=True
)
```

Take a snapshot of the active viewport.

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `animation` | bool | False | Render animation range |
| `render_keyed_only` | bool | False | Only render keyed frames |
| `sequencer` | bool | False | Use sequencer's OpenGL display |
| `write_still` | bool | False | Save rendered image to output path |
| `view_context` | bool | True | Use current 3D view (else scene settings) |

---

## 12. bpy.ops.object

### mode_set

```python
bpy.ops.object.mode_set(*, mode='OBJECT', toggle=False)
```

Sets the object interaction mode.

**mode values:** `'OBJECT'`, `'EDIT'`, `'SCULPT'`, `'VERTEX_PAINT'`, `'WEIGHT_PAINT'`,
`'TEXTURE_PAINT'`, `'PARTICLE_EDIT'`, `'POSE'`, `'SCULPT_CURVES'`, `'PAINT_GREASE_PENCIL'`,
`'SCULPT_GREASE_PENCIL'`, `'WEIGHT_GREASE_PENCIL'`, `'EDIT_GREASE_PENCIL'`

### select_all

```python
bpy.ops.object.select_all(*, action='TOGGLE')
```

Change selection of all visible objects.

**action values:** `'TOGGLE'`, `'SELECT'`, `'DESELECT'`, `'INVERT'`

### empty_image_add

```python
bpy.ops.object.empty_image_add(
    *, filepath='', relative_path=True,
    name='', align='WORLD',
    location=(0.0, 0.0, 0.0), rotation=(0.0, 0.0, 0.0), scale=(0.0, 0.0, 0.0),
    background=False
)
```

Add an empty image type to scene with data.

### join

```python
bpy.ops.object.join()
```

Join selected objects into the active object. No parameters.

### delete

```python
bpy.ops.object.delete(*, use_global=False, confirm=True)
```

Delete selected objects.

---

## 13. bpy.ops.preferences

### addon_show

```python
bpy.ops.preferences.addon_show(*, module='')
```

Show add-on preferences.

| Parameter | Type | Description |
|-----------|------|-------------|
| `module` | str | Module name of the add-on to expand |

---

## 14. bpy.types.Object

### Key Properties

| Property | Type | Description |
|----------|------|-------------|
| `type` | enum (readonly) | Object type: `'MESH'`, `'CURVE'`, `'SURFACE'`, `'META'`, `'FONT'`, `'ARMATURE'`, `'LATTICE'`, `'EMPTY'`, `'GREASEPENCIL'`, `'CAMERA'`, `'LIGHT'`, `'SPEAKER'`, `'LIGHT_PROBE'`, `'VOLUME'`, `'CURVES'`, `'POINTCLOUD'` |
| `data` | `ID` | Object data (e.g. `GreasePencil` for GP objects) |
| `location` | `Vector` (3) | Object location |
| `rotation_euler` | `Euler` (3) | Object rotation |
| `scale` | `Vector` (3) | Object scale |
| `matrix_world` | `Matrix` 4x4 | World transformation matrix |
| `parent` | `Object` | Parent object |
| `children` | collection of `Object` | Child objects |
| `modifiers` | collection of `Modifier` | Object modifiers |

### Key Methods

```python
Object.select_set(state, *, view_layer=None)
    # state: bool -- Select or deselect
    # view_layer: ViewLayer (optional) -- Defaults to active view layer

Object.select_get(*, view_layer=None) -> bool
    # Check selection state
```

**Important:** For Grease Pencil objects, `obj.type == 'GREASEPENCIL'` (not `'GPENCIL'`).
This is the Blender 5.0 / Grease Pencil v3 type string.

---

## 15. bpy.types.Scene

### Key Properties

| Property | Type | Description |
|----------|------|-------------|
| `frame_current` | int | Current frame (**read via this, write via `frame_set()`**) |
| `frame_start` | int | First frame of playback/render range |
| `frame_end` | int | Last frame of playback/render range |
| `render` | `RenderSettings` (readonly) | Render settings |
| `camera` | `Object` | Active camera |
| `world` | `World` | Active world |
| `objects` | collection of `Object` (readonly) | All objects |
| `collection` | `Collection` (readonly) | Scene master collection |
| `cursor` | `View3DCursor` | 3D cursor |
| `tool_settings` | `ToolSettings` (readonly) | Tool settings |

### Key Methods

```python
Scene.frame_set(frame, *, subframe=0.0)
    # Set frame and update all objects/view layers immediately
    # frame: int [-1048574, 1048574]
    # subframe: float [0, 1]
```

**Important:** Use `frame_set()` instead of assigning `frame_current` directly,
to ensure proper scene update.

---

## 16. RenderSettings & ImageFormatSettings

### bpy.types.RenderSettings

| Property | Type | Description |
|----------|------|-------------|
| `filepath` | str | Output file path (supports `//` prefix for blend-relative) |
| `image_settings` | `ImageFormatSettings` (readonly) | Image format settings |
| `resolution_x` | int | Horizontal resolution |
| `resolution_y` | int | Vertical resolution |
| `resolution_percentage` | int | Resolution scale percentage |
| `fps` | int | Frames per second |
| `fps_base` | float | FPS base factor |
| `use_file_extension` | bool | Append format extension to file path |

### bpy.types.ImageFormatSettings

| Property | Type | Description |
|----------|------|-------------|
| `file_format` | enum | `'PNG'`, `'JPEG'`, `'BMP'`, `'TARGA'`, `'OPEN_EXR'`, etc. |
| `color_mode` | enum | `'BW'`, `'RGB'`, `'RGBA'` |
| `color_depth` | enum | `'8'`, `'16'`, `'32'` |
| `compression` | int [0,100] | PNG compression level |
| `quality` | int [0,100] | JPEG quality |

---

## 17. bpy.context

Global read-only context. Properties vary by area/mode.

### Always Available

| Property | Type | Description |
|----------|------|-------------|
| `scene` | `Scene` (readonly) | Current scene |
| `view_layer` | `ViewLayer` (readonly) | Active view layer |
| `window` | `Window` (readonly) | Current window |
| `window_manager` | `WindowManager` (readonly) | Window manager |
| `preferences` | `Preferences` (readonly) | User preferences |
| `mode` | enum (readonly) | Current interaction mode |

### 3D View Context

| Property | Type | Description |
|----------|------|-------------|
| `active_object` | `Object` | Active object (read/write) |
| `selected_objects` | list of `Object` (readonly) | Selected objects |
| `object` | `Object` (readonly) | Convenience alias for `active_object` |

### Accessing Addon Preferences

```python
prefs = context.preferences.addons[__package__].preferences
api_key = prefs.api_key
```

### ViewLayer

```python
context.view_layer.objects.active  # Active object (read/write)
context.view_layer.objects         # All objects in view layer
```

---

## 18. bpy.path

Utility functions for Blender-specific path handling.

```python
bpy.path.abspath(path, *, start=None, library=None) -> str
    # Convert "//" relative path to absolute path
    # start: optional base path (defaults to current blend file directory)

bpy.path.relpath(path, *, start=None) -> str
    # Convert absolute path to "//" relative path

bpy.path.basename(path) -> str
    # Like os.path.basename but skips "//" prefix

bpy.path.clean_name(name, *, replace='_') -> str
    # Replace non-alphanumeric characters

bpy.path.ensure_ext(filepath, ext, *, case_sensitive=False) -> str
    # Add extension if missing (ext should start with ".")

bpy.path.display_name(name, *, has_ext=True, title_case=True) -> str
    # Create UI-friendly display name from filename
```

---

## 19. bpy.data

Global data access. Contains all loaded data-blocks.

```python
bpy.data.filepath -> str
    # Path of the current .blend file ("" if unsaved)

bpy.data.objects -> collection of Object
bpy.data.scenes -> collection of Scene
bpy.data.grease_pencils -> collection of GreasePencil
bpy.data.materials -> collection of Material
bpy.data.images -> collection of Image
```

### Creating / Removing Data

```python
# Grease Pencil data
gp_data = bpy.data.grease_pencils.new("MyGP")
bpy.data.grease_pencils.remove(gp_data)
```

---

## 20. bpy.utils

Utility functions for addon management.

```python
bpy.utils.register_class(cls)
    # Register a Blender class (Operator, Panel, PropertyGroup, etc.)

bpy.utils.unregister_class(cls)
    # Unregister a previously registered class
```

---

## Appendix: Common Patterns for Tweenybird

### Checking for Grease Pencil Object

```python
obj = context.active_object
if obj is None or obj.type != 'GREASEPENCIL':
    return False  # in poll()
```

### Iterating Keyframes

```python
gp_data = gp_obj.data
layer = gp_data.layers.active
for frame in layer.frames:
    print(f"Frame {frame.frame_number}, selected={frame.select}")
```

### Creating/Removing Frames

```python
layer = gp_obj.data.layers.active

# Create
new_frame = layer.frames.new(frame_number=10)

# Remove
layer.frames.remove(frame_number=10)

# Copy
copied = layer.frames.copy(from_frame_number=1, to_frame_number=15)

# Move
moved = layer.frames.move(from_frame_number=10, to_frame_number=20)
```

### Exporting Frame as PNG (Viewport Render)

```python
original_filepath = scene.render.filepath
original_format = scene.render.image_settings.file_format
original_color_mode = scene.render.image_settings.color_mode

try:
    scene.render.filepath = str(output_path)
    scene.render.image_settings.file_format = 'PNG'
    scene.render.image_settings.color_mode = 'RGBA'
    bpy.ops.render.opengl(write_still=True)
finally:
    scene.render.filepath = original_filepath
    scene.render.image_settings.file_format = original_format
    scene.render.image_settings.color_mode = original_color_mode
```

### Tracing an Image to GP Strokes

```python
# Add image empty
bpy.ops.object.empty_image_add(filepath=str(png_path), location=gp_obj.location)
trace_empty = context.active_object

# Trace image into GP strokes
bpy.ops.grease_pencil.trace_image(
    target='NEW',
    radius=0.01,
    threshold=0.5,
    turnpolicy='MINORITY',
    mode='SINGLE',
    use_current_frame=True,
)

# Join traced GP into target
bpy.ops.object.select_all(action='DESELECT')
traced_gp.select_set(True)
gp_obj.select_set(True)
context.view_layer.objects.active = gp_obj
bpy.ops.object.join()

# Clean up image empty
bpy.ops.object.select_all(action='DESELECT')
trace_empty.select_set(True)
bpy.ops.object.delete()
```

### Switching Frames Safely

```python
original_frame = context.scene.frame_current
context.scene.frame_set(target_frame)
# ... do work ...
context.scene.frame_set(original_frame)  # Restore
```

### Saving/Restoring Editor State

```python
original_mode = context.mode
if original_mode != 'OBJECT':
    bpy.ops.object.mode_set(mode='OBJECT')
# ... do work ...
# Restore mode if needed
```

### Accessing Blend File Directory

```python
if bpy.data.filepath:
    blend_dir = Path(bpy.path.abspath("//"))
else:
    blend_dir = Path.home()  # Unsaved file fallback
```
