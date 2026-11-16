//! The bundled serif reaches an app through `isonomic_app::ui` alone: the
//! theme names it, and the font files Isonomic UI's `init` registers carry
//! it beside the sans and the mono.
//!
//! The headless test platform's text system lists no fonts, so this checks
//! the files `init` hands to `add_fonts` rather than the platform's list.

use isonomic_app::ui;

use ui::component::ActiveTheme;
use ui::TestAppContext;

/// Whether a font file names `family`, in its name table's single-byte or
/// UTF-16 form.
fn names(file: &[u8], family: &str) -> bool {
    let wide: Vec<u8> = family.encode_utf16().flat_map(u16::to_be_bytes).collect();
    let has = |needle: &[u8]| file.windows(needle.len()).any(|w| w == needle);
    has(family.as_bytes()) || has(&wide)
}

#[test]
fn the_bundled_files_carry_every_family() {
    let files = ui::assets::fonts();
    for family in [ui::assets::SANS, ui::assets::MONO, ui::assets::SERIF] {
        let count = files.iter().filter(|f| names(f, family)).count();
        assert!(count >= 2, "{family}: {count} files");
    }
}

#[ui::test]
fn the_theme_names_the_bundled_serif(cx: &mut TestAppContext) {
    cx.update(|cx| {
        ui::init(cx);
        assert_eq!(cx.theme().serif_font_family.as_ref(), ui::assets::SERIF);
    });
}
