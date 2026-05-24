"""Shared PyInstaller configuration helpers.

Import from platform-specific .spec files to avoid duplication of
Analysis/PYZ/EXE boilerplate.
"""

import os


def make_analysis(script_path, binaries, runtime_hook="packaging/runtime_hook_mpv.py"):
    """Build a PyInstaller ``Analysis`` with standard settings.

    Parameters
    ----------
    script_path:
        Path to the entry-point script, resolved relative to *this* file.
    binaries:
        Sequence of ``(source_path, dest_name)`` tuples for bundled libs.
    runtime_hook:
        Path to the runtime hook (resolved relative to SPECPATH).

    Returns
    -------
    Analysis
    """
    # Resolve the script path relative to the spec file directory
    base_dir = os.path.dirname(os.path.abspath(__file__))
    full_script = os.path.join(base_dir, script_path)

    # Resolve runtime hook relative to the spec file
    hook = os.path.join(base_dir, runtime_hook)

    return Analysis(
        [full_script],
        pathex=[],
        binaries=binaries,
        datas=[],
        hiddenimports=["mpv"],
        hookspath=[],
        runtime_hooks=[hook],
        hooksconfig={},
        excludes=[],
        noarchive=False,
    )


def make_pyz(analysis):
    """Build a PyInstaller ``PYZ`` archive from an *analysis*."""
    return PYZ(analysis.pure)


def make_exe(pyz, analysis, icon=None, console=False, collect=False):
    """Build a PyInstaller ``EXE``.

    Parameters
    ----------
    collect:
        If *True* (macOS), use ``exclude_binaries=True`` and return a
        ``COLLECT`` object as well.
    """
    if collect:
        exe = EXE(
            pyz,
            analysis.scripts,
            [],
            exclude_binaries=True,
            name="jorja-clipper",
            debug=False,
            bootloader_ignore_signals=False,
            strip=False,
            console=console,
            disable_windowed_traceback=False,
            icon=icon,
        )
        coll = COLLECT(
            exe,
            analysis.binaries,
            analysis.datas,
            strip=False,
            upx=True,
            upx_exclude=[],
            name="jorja-clipper",
        )
        return exe, coll

    return EXE(
        pyz,
        analysis.scripts,
        analysis.binaries,
        analysis.datas,
        [],
        name="jorja-clipper",
        debug=False,
        bootloader_ignore_signals=False,
        strip=False,
        upx=True,
        upx_exclude=[],
        runtime_tmpdir=None,
        console=console,
        disable_windowed_traceback=False,
        icon=icon,
    )
