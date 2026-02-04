"""
UI panels for GP AI Inbetween.
"""

import bpy


class GPAI_PT_MainPanel(bpy.types.Panel):
    """Main UI panel in the sidebar."""
    bl_label = "GP AI Inbetween"
    bl_idname = "GPAI_PT_main_panel"
    bl_space_type = 'VIEW_3D'
    bl_region_type = 'UI'
    bl_category = 'GP AI'

    def draw(self, context):
        layout = self.layout

        # Check for GP object
        obj = context.active_object
        if obj is None or obj.type != 'GREASEPENCIL':
            layout.label(text="Select a Grease Pencil object", icon='INFO')
            return

        # Generate section
        box = layout.box()
        box.label(text="Generate Inbetweens", icon='PLAY')

        row = box.row()
        row.scale_y = 1.5
        row.operator("gpai.generate_inbetweens", icon='RENDER_ANIMATION')

        # Instructions
        box.label(text="Select 2 keyframes in the timeline,")
        box.label(text="then click Generate.")

        # Settings shortcut
        layout.separator()
        layout.operator("gpai.open_config", icon='PREFERENCES')


class GPAI_PT_ResultsPanel(bpy.types.Panel):
    """Panel for reviewing generated frames."""
    bl_label = "Review Frames"
    bl_idname = "GPAI_PT_results_panel"
    bl_space_type = 'VIEW_3D'
    bl_region_type = 'UI'
    bl_category = 'GP AI'
    bl_options = {'DEFAULT_CLOSED'}

    def draw(self, context):
        layout = self.layout

        # Current frame info
        box = layout.box()
        box.label(text=f"Frame: {context.scene.frame_current}", icon='TIME')

        if context.scene.gpai.last_motion_type:
            box.label(text=f"Motion: {context.scene.gpai.last_motion_type}")

        if context.scene.gpai.character_name:
            box.label(text=f"Character: {context.scene.gpai.character_name}")

        # Accept/Reject buttons
        row = layout.row(align=True)
        row.scale_y = 1.3

        accept = row.operator("gpai.accept_frame", text="Accept", icon='CHECKMARK')
        reject = row.operator("gpai.reject_frame", text="Reject", icon='X')

        # Navigation help
        layout.separator()
        layout.label(text="Use ← → to navigate frames", icon='INFO')


class GPAI_PT_StatsPanel(bpy.types.Panel):
    """Panel for viewing statistics."""
    bl_label = "Statistics"
    bl_idname = "GPAI_PT_stats_panel"
    bl_space_type = 'VIEW_3D'
    bl_region_type = 'UI'
    bl_category = 'GP AI'
    bl_options = {'DEFAULT_CLOSED'}

    def draw(self, context):
        layout = self.layout

        layout.operator("gpai.show_stats", icon='GRAPH')

        layout.separator()
        layout.label(text="Track your acceptance rate")
        layout.label(text="to improve future generations.")
