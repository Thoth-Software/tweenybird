"""
Blender addon for AI-assisted Grease Pencil inbetweening.
Calls Rust binary for all heavy lifting.
"""

bl_info = {
    "name": "GP AI Inbetween",
    "author": "GP AI Inbetween Contributors",
    "version": (0, 1, 0),
    "blender": (4, 0, 0),
    "location": "View3D > Sidebar > GP AI",
    "description": "AI-assisted inbetweening for Grease Pencil (Rust-powered)",
    "warning": "Requires Replicate API key for AI generation",
    "doc_url": "https://github.com/your-repo/gp-ai-inbetween",
    "category": "Animation",
}

import bpy
from bpy.props import StringProperty, IntProperty, FloatProperty, BoolProperty

from . import operators
from . import ui


# Addon preferences
class GPAI_Preferences(bpy.types.AddonPreferences):
    bl_idname = __name__

    api_key: StringProperty(
        name="Replicate API Key",
        description="Your Replicate API key (get one at replicate.com)",
        default="",
        subtype='PASSWORD',
    )

    auto_accept_threshold: FloatProperty(
        name="Auto-Accept Threshold",
        description="Confidence threshold for auto-accepting frames (0.0-1.0)",
        default=0.85,
        min=0.0,
        max=1.0,
    )

    default_num_frames: IntProperty(
        name="Default Number of Frames",
        description="Default number of inbetween frames to generate",
        default=4,
        min=1,
        max=16,
    )

    verbose_logging: BoolProperty(
        name="Verbose Logging",
        description="Enable detailed logging in the console",
        default=False,
    )

    def draw(self, context):
        layout = self.layout

        box = layout.box()
        box.label(text="API Configuration", icon='WORLD')
        box.prop(self, "api_key")
        if not self.api_key:
            box.label(text="⚠ API key required for generation", icon='ERROR')

        box = layout.box()
        box.label(text="Generation Settings", icon='SETTINGS')
        box.prop(self, "auto_accept_threshold", slider=True)
        box.prop(self, "default_num_frames")
        box.prop(self, "verbose_logging")


# Scene properties for per-scene settings
class GPAI_SceneProperties(bpy.types.PropertyGroup):
    character_name: StringProperty(
        name="Character",
        description="Character name for tracking (helps improve future generations)",
        default="",
    )

    last_motion_type: StringProperty(
        name="Last Motion Type",
        description="Motion type detected in last generation",
        default="",
    )

    last_output_dir: StringProperty(
        name="Last Output Directory",
        description="Directory used for last generation",
        default="",
        subtype='DIR_PATH',
    )


classes = [
    GPAI_Preferences,
    GPAI_SceneProperties,
    operators.GPAI_OT_GenerateInbetweens,
    operators.GPAI_OT_AcceptFrame,
    operators.GPAI_OT_RejectFrame,
    operators.GPAI_OT_ShowStats,
    operators.GPAI_OT_OpenConfig,
    ui.GPAI_PT_MainPanel,
    ui.GPAI_PT_ResultsPanel,
    ui.GPAI_PT_StatsPanel,
]


def register():
    for cls in classes:
        bpy.utils.register_class(cls)

    bpy.types.Scene.gpai = bpy.props.PointerProperty(type=GPAI_SceneProperties)


def unregister():
    del bpy.types.Scene.gpai

    for cls in reversed(classes):
        bpy.utils.unregister_class(cls)


if __name__ == "__main__":
    register()
