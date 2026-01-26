"""
Operators that call the Rust binary.
"""

import bpy
import subprocess
import tempfile
import json
import platform
from pathlib import Path
from bpy.props import IntProperty, StringProperty, EnumProperty


def get_binary_path():
    """Find the Rust binary based on platform."""
    addon_dir = Path(__file__).parent
    bin_dir = addon_dir / "bin"

    system = platform.system()

    if system == "Windows":
        binary = bin_dir / "gp_inbetween.exe"
    elif system == "Darwin":
        binary = bin_dir / "gp_inbetween_mac"
    else:
        binary = bin_dir / "gp_inbetween"

    if not binary.exists():
        raise FileNotFoundError(
            f"GP AI binary not found at {binary}. "
            "Please ensure the addon is installed correctly."
        )

    return str(binary)


def get_preferences():
    """Get addon preferences."""
    return bpy.context.preferences.addons[__package__].preferences


def write_config_file(config_path: Path, api_key: str, threshold: float):
    """Write a temporary config file with current settings."""
    config = f"""[api]
backend = "replicate"
endpoint = "http://localhost:8000/generate"
api_key = "{api_key}"
replicate_model = "fofr/tooncrafter:0d5c6b3a4e0d6b8a9b8e7d6c5b4a3f2e1d0c9b8a"
style_strength = 0.8
timeout_secs = 180

[preprocessing]
cleanup_enabled = true
target_resolution = 1024
normalize_resolution = true
min_stroke_length = 5.0

auto_accept_threshold = {threshold}
"""
    config_path.write_text(config)


class GPAI_OT_GenerateInbetweens(bpy.types.Operator):
    """Generate AI inbetweens between selected keyframes"""
    bl_idname = "gpai.generate_inbetweens"
    bl_label = "Generate Inbetweens"
    bl_options = {'REGISTER', 'UNDO'}

    num_frames: IntProperty(
        name="Number of Frames",
        default=4,
        min=1,
        max=16,
        description="Number of inbetween frames to generate",
    )

    character: StringProperty(
        name="Character",
        default="",
        description="Character name for tracking (optional)",
    )

    @classmethod
    def poll(cls, context):
        # Check that we have a Grease Pencil object selected
        obj = context.active_object
        if obj is None or obj.type != 'GPENCIL':
            return False
        return True

    def invoke(self, context, event):
        prefs = get_preferences()
        self.num_frames = prefs.default_num_frames

        # Use scene character name if set
        if context.scene.gpai.character_name:
            self.character = context.scene.gpai.character_name

        return context.window_manager.invoke_props_dialog(self)

    def draw(self, context):
        layout = self.layout
        layout.prop(self, "num_frames")
        layout.prop(self, "character")

        prefs = get_preferences()
        if not prefs.api_key:
            layout.label(text="⚠ Set API key in addon preferences!", icon='ERROR')

    def execute(self, context):
        prefs = get_preferences()

        if not prefs.api_key:
            self.report({'ERROR'}, "Replicate API key not set. Check addon preferences.")
            return {'CANCELLED'}

        try:
            binary = get_binary_path()
        except FileNotFoundError as e:
            self.report({'ERROR'}, str(e))
            return {'CANCELLED'}

        # Get the active GP object
        gp_obj = context.active_object
        if gp_obj.type != 'GPENCIL':
            self.report({'ERROR'}, "Active object is not a Grease Pencil object")
            return {'CANCELLED'}

        # Get selected keyframes
        keyframes = self.get_selected_keyframes(context, gp_obj)
        if len(keyframes) < 2:
            self.report({'ERROR'}, "Select at least 2 keyframes in the timeline")
            return {'CANCELLED'}

        # Use first and last selected frames
        frame_a_num = min(keyframes)
        frame_b_num = max(keyframes)

        if frame_b_num - frame_a_num < 2:
            self.report({'ERROR'}, "Keyframes must be at least 2 frames apart")
            return {'CANCELLED'}

        try:
            with tempfile.TemporaryDirectory() as tmpdir:
                tmpdir = Path(tmpdir)

                png_a = tmpdir / "frame_a.png"
                png_b = tmpdir / "frame_b.png"
                output_dir = tmpdir / "generated"
                output_dir.mkdir()
                config_path = tmpdir / "config.toml"

                # Write config
                write_config_file(config_path, prefs.api_key, prefs.auto_accept_threshold)

                # Export frames
                self.export_gp_frame_to_png(context, gp_obj, frame_a_num, png_a)
                self.export_gp_frame_to_png(context, gp_obj, frame_b_num, png_b)

                # Build command
                cmd = [
                    binary,
                    "generate",
                    "--frame-a", str(png_a),
                    "--frame-b", str(png_b),
                    "--num-frames", str(self.num_frames),
                    "--output-dir", str(output_dir),
                    "--config", str(config_path),
                ]

                if self.character:
                    cmd.extend(["--character", self.character])

                if prefs.verbose_logging:
                    cmd.insert(1, "--verbose")

                # Run generation
                self.report({'INFO'}, "Generating frames... (this may take a minute)")

                result = subprocess.run(
                    cmd,
                    capture_output=True,
                    text=True,
                    timeout=300,  # 5 minute timeout
                )

                if result.returncode != 0:
                    error_msg = result.stderr or result.stdout or "Unknown error"
                    self.report({'ERROR'}, f"Generation failed: {error_msg}")
                    return {'CANCELLED'}

                # Read metadata
                metadata_path = output_dir / "metadata.json"
                if metadata_path.exists():
                    with open(metadata_path) as f:
                        metadata = json.load(f)
                else:
                    metadata = {"confidence_scores": [], "auto_accept": []}

                # Import generated frames
                generated_pngs = sorted(output_dir.glob("*.png"))

                if not generated_pngs:
                    self.report({'ERROR'}, "No frames were generated")
                    return {'CANCELLED'}

                # Calculate frame spacing
                total_gap = frame_b_num - frame_a_num - 1
                frame_positions = self.calculate_frame_positions(
                    frame_a_num, frame_b_num, len(generated_pngs)
                )

                for i, (png_path, frame_num) in enumerate(zip(generated_pngs, frame_positions)):
                    confidence = metadata.get("confidence_scores", [0.0] * len(generated_pngs))[i]
                    auto_accept = metadata.get("auto_accept", [False] * len(generated_pngs))[i]

                    self.import_png_to_gp_frame(context, gp_obj, png_path, frame_num)

                    if auto_accept:
                        self.log_acceptance(binary, frame_num, confidence)

                # Store info for later
                context.scene.gpai.last_motion_type = metadata.get("motion_type", "unknown")
                context.scene.gpai.character_name = self.character

                self.report(
                    {'INFO'},
                    f"Generated {len(generated_pngs)} frames between {frame_a_num} and {frame_b_num}"
                )
                return {'FINISHED'}

        except subprocess.TimeoutExpired:
            self.report({'ERROR'}, "Generation timed out (5 minutes). Try with fewer frames.")
            return {'CANCELLED'}
        except Exception as e:
            self.report({'ERROR'}, f"Error: {str(e)}")
            return {'CANCELLED'}

    def get_selected_keyframes(self, context, gp_obj):
        """Get list of selected keyframe numbers from the active GP layer."""
        keyframes = []

        gp_data = gp_obj.data
        if not gp_data.layers.active:
            return keyframes

        layer = gp_data.layers.active

        for frame in layer.frames:
            if frame.select:
                keyframes.append(frame.frame_number)

        # If no frames explicitly selected, try to use current frame and next keyframe
        if not keyframes:
            current = context.scene.frame_current
            frame_numbers = sorted([f.frame_number for f in layer.frames])

            if frame_numbers:
                # Find current or previous keyframe
                prev_frames = [f for f in frame_numbers if f <= current]
                next_frames = [f for f in frame_numbers if f > current]

                if prev_frames and next_frames:
                    keyframes = [prev_frames[-1], next_frames[0]]

        return sorted(keyframes)

    def calculate_frame_positions(self, start_frame, end_frame, num_frames):
        """Calculate evenly-spaced frame positions between start and end."""
        if num_frames == 0:
            return []

        gap = end_frame - start_frame
        step = gap / (num_frames + 1)

        positions = []
        for i in range(1, num_frames + 1):
            pos = start_frame + int(i * step)
            positions.append(pos)

        return positions

    def export_gp_frame_to_png(self, context, gp_obj, frame_num, output_path):
        """Export a GP frame to PNG file."""
        # Store original state
        original_frame = context.scene.frame_current

        # Set to target frame
        context.scene.frame_set(frame_num)

        # Set up render settings for GP export
        scene = context.scene
        original_filepath = scene.render.filepath
        original_format = scene.render.image_settings.file_format
        original_color_mode = scene.render.image_settings.color_mode

        try:
            scene.render.filepath = str(output_path)
            scene.render.image_settings.file_format = 'PNG'
            scene.render.image_settings.color_mode = 'RGBA'

            # Render using OpenGL (shows GP strokes)
            bpy.ops.render.opengl(write_still=True)

        finally:
            # Restore original state
            scene.render.filepath = original_filepath
            scene.render.image_settings.file_format = original_format
            scene.render.image_settings.color_mode = original_color_mode
            context.scene.frame_set(original_frame)

    def import_png_to_gp_frame(self, context, gp_obj, png_path, frame_num):
        """Import PNG as a new GP frame."""
        gp_data = gp_obj.data
        layer = gp_data.layers.active

        if layer is None:
            self.report({'WARNING'}, "No active GP layer")
            return

        # Check if frame already exists
        existing_frame = None
        for frame in layer.frames:
            if frame.frame_number == frame_num:
                existing_frame = frame
                break

        if existing_frame:
            # Remove existing frame content
            layer.frames.remove(existing_frame)

        # Create new frame
        new_frame = layer.frames.new(frame_num)

        # For now, we import as an image reference
        # A more sophisticated approach would trace the image to GP strokes
        # This is a placeholder - real implementation would use:
        # - bpy.ops.gpencil.trace_image() for auto-tracing
        # - Or manual stroke creation from contours

        # Load the image as a reference
        img = bpy.data.images.load(str(png_path))
        img.name = f"gpai_frame_{frame_num}"

        # Create an empty with the image as reference
        # (This is temporary - you'd want to trace to strokes)
        bpy.ops.object.empty_add(type='IMAGE', location=gp_obj.location)
        empty = context.active_object
        empty.data = img
        empty.name = f"GPAI_Ref_{frame_num}"
        empty.empty_display_size = 1.0

        # Set the GP object back as active
        context.view_layer.objects.active = gp_obj

    def log_acceptance(self, binary, frame_num, confidence):
        """Log auto-acceptance of a frame."""
        try:
            subprocess.run([
                binary,
                "accept",
                "--frame-number", str(frame_num),
                "--character", self.character or "unknown",
                "--motion-type", "unknown",
                "--auto", "true",
                "--confidence", str(confidence),
            ], capture_output=True, timeout=5)
        except Exception:
            pass  # Don't fail the whole operation for logging errors


class GPAI_OT_AcceptFrame(bpy.types.Operator):
    """Accept the AI-generated frame at current position"""
    bl_idname = "gpai.accept_frame"
    bl_label = "Accept Frame"
    bl_options = {'REGISTER', 'UNDO'}

    def execute(self, context):
        try:
            binary = get_binary_path()
        except FileNotFoundError as e:
            self.report({'ERROR'}, str(e))
            return {'CANCELLED'}

        frame_num = context.scene.frame_current
        character = context.scene.gpai.character_name or "unknown"
        motion_type = context.scene.gpai.last_motion_type or "unknown"

        subprocess.run([
            binary,
            "accept",
            "--frame-number", str(frame_num),
            "--character", character,
            "--motion-type", motion_type,
        ], capture_output=True)

        self.report({'INFO'}, f"Accepted frame {frame_num}")
        return {'FINISHED'}


class GPAI_OT_RejectFrame(bpy.types.Operator):
    """Reject the AI-generated frame at current position"""
    bl_idname = "gpai.reject_frame"
    bl_label = "Reject Frame"
    bl_options = {'REGISTER', 'UNDO'}

    issues: EnumProperty(
        name="Issues",
        description="What's wrong with this frame?",
        items=[
            ('artifacts', "Artifacts", "Visual artifacts or glitches"),
            ('wrong_motion', "Wrong Motion", "Motion doesn't match expected path"),
            ('style_mismatch', "Style Mismatch", "Doesn't match art style"),
            ('missing_parts', "Missing Parts", "Parts of character are missing"),
            ('extra_parts', "Extra Parts", "Unwanted elements appeared"),
            ('proportion', "Proportion Issues", "Character proportions are off"),
            ('other', "Other", "Other issues"),
        ],
        default='artifacts',
    )

    def invoke(self, context, event):
        return context.window_manager.invoke_props_dialog(self)

    def draw(self, context):
        layout = self.layout
        layout.prop(self, "issues")

    def execute(self, context):
        try:
            binary = get_binary_path()
        except FileNotFoundError as e:
            self.report({'ERROR'}, str(e))
            return {'CANCELLED'}

        frame_num = context.scene.frame_current
        character = context.scene.gpai.character_name or "unknown"
        motion_type = context.scene.gpai.last_motion_type or "unknown"

        subprocess.run([
            binary,
            "reject",
            "--frame-number", str(frame_num),
            "--character", character,
            "--motion-type", motion_type,
            "--issues", self.issues,
        ], capture_output=True)

        self.report({'INFO'}, f"Rejected frame {frame_num}")
        return {'FINISHED'}


class GPAI_OT_ShowStats(bpy.types.Operator):
    """Show generation statistics"""
    bl_idname = "gpai.show_stats"
    bl_label = "Show Statistics"

    def execute(self, context):
        try:
            binary = get_binary_path()
        except FileNotFoundError as e:
            self.report({'ERROR'}, str(e))
            return {'CANCELLED'}

        result = subprocess.run(
            [binary, "stats"],
            capture_output=True,
            text=True,
        )

        def draw_popup(self, context):
            for line in result.stdout.split('\n'):
                if line.strip():
                    self.layout.label(text=line)

        context.window_manager.popup_menu(draw_popup, title="GP AI Statistics", icon='INFO')
        return {'FINISHED'}


class GPAI_OT_OpenConfig(bpy.types.Operator):
    """Open addon preferences"""
    bl_idname = "gpai.open_config"
    bl_label = "Open Settings"

    def execute(self, context):
        bpy.ops.preferences.addon_show(module=__package__)
        return {'FINISHED'}
