use super::*;

#[test]
fn package_rejects_actual_expansion_above_forged_entry_size() {
    use std::io::Write;
    use zip_slide::{write::SimpleFileOptions, CompressionMethod, ZipWriter};

    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    for name in [
        "[Content_Types].xml",
        "ppt/presentation.xml",
        "ppt/_rels/presentation.xml.rels",
    ] {
        writer
            .start_file(
                name,
                SimpleFileOptions::default().compression_method(CompressionMethod::Deflated),
            )
            .unwrap();
        if name == "[Content_Types].xml" {
            std::io::copy(
                &mut std::io::repeat(b'x').take(MAX_ENTRY_BYTES + 1),
                &mut writer,
            )
            .unwrap();
        } else {
            writer.write_all(b"x").unwrap();
        }
    }
    let mut bytes = writer.finish().unwrap().into_inner();
    // Lie in both headers while keeping compressed bytes and their CRC intact.
    let central = bytes.windows(4).position(|b| b == b"PK\x01\x02").unwrap();
    bytes[22..26].copy_from_slice(&1_u32.to_le_bytes());
    bytes[central + 24..central + 28].copy_from_slice(&1_u32.to_le_bytes());
    validate_package(&bytes).unwrap();
    let Err(error) = read_package(&bytes) else {
        panic!("package exceeded the actual decompression limit");
    };
    assert_eq!(error.kind, FailureKind::TooLarge);
    assert_eq!(error.code, "powerpoint_too_large");

    // Insertion targets must use the same streaming limits as imports.
    let error = super::super::insertion::insert(&bytes, &bytes, 0).unwrap_err();
    assert_eq!(
        error.downcast_ref::<SlideTemplateError>().unwrap().code,
        "powerpoint_too_large"
    );
}

#[test]
fn part_expansion_counts_actual_bytes_against_the_remaining_package_budget() {
    let mut total = MAX_EXPANDED_BYTES - 8;
    assert_eq!(
        read_part(Cursor::new(b"first"), 1, &mut total).unwrap(),
        b"first"
    );
    assert_eq!(total, MAX_EXPANDED_BYTES - 3);

    let mut next = Cursor::new(b"oversized");
    let error = read_part(&mut next, 1, &mut total).unwrap_err();
    assert_eq!(error.kind, FailureKind::TooLarge);
    assert_eq!(
        next.position(),
        4,
        "stop one byte past the remaining budget"
    );
}

#[test]
fn part_expansion_accepts_exact_budget_and_checks_end_of_stream() {
    let mut total = MAX_EXPANDED_BYTES - 3;
    assert_eq!(
        read_part(Cursor::new(b"fit"), 3, &mut total).unwrap(),
        b"fit"
    );
    assert_eq!(total, MAX_EXPANDED_BYTES);
    assert!(read_part(std::io::empty(), 0, &mut total)
        .unwrap()
        .is_empty());
    let mut next = Cursor::new(b"extra");
    assert_eq!(
        read_part(&mut next, 0, &mut total).unwrap_err().kind,
        FailureKind::TooLarge
    );
    assert_eq!(next.position(), 1);
}

const THEME_XML: &str = r#"
        <a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
          <a:themeElements>
            <a:clrScheme name="Custom">
              <a:dk1><a:srgbClr val="070154"/></a:dk1>
              <a:lt1><a:srgbClr val="FFFFFF"/></a:lt1>
              <a:accent2><a:srgbClr val="50658E"/></a:accent2>
              <a:accent3><a:srgbClr val="CED7E6"/></a:accent3>
            </a:clrScheme>
            <a:fontScheme name="Custom">
              <a:majorFont><a:latin typeface="Arial"/></a:majorFont>
              <a:minorFont><a:latin typeface="Arial"/></a:minorFont>
            </a:fontScheme>
          </a:themeElements>
        </a:theme>
    "#;

const CONNECTOR_THEME_XML: &str = r#"
        <a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
          <a:themeElements>
            <a:clrScheme name="Custom">
              <a:dk1><a:srgbClr val="070154"/></a:dk1>
            </a:clrScheme>
            <a:fmtScheme name="Custom">
              <a:lnStyleLst>
                <a:ln w="6350"><a:solidFill><a:schemeClr val="dk1"/></a:solidFill></a:ln>
              </a:lnStyleLst>
            </a:fmtScheme>
          </a:themeElements>
        </a:theme>
    "#;

const ELBOW_CONNECTOR_XML: &str = r#"
      <p:cxnSp xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"
               xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
        <p:nvCxnSpPr><p:cNvPr id="42" name="Elbow Connector"/></p:nvCxnSpPr>
        <p:spPr>
          <a:xfrm flipV="1">
            <a:off x="0" y="0"/><a:ext cx="952500" cy="1905000"/>
          </a:xfrm>
          <a:prstGeom prst="bentConnector3">
            <a:avLst><a:gd name="adj1" fmla="val 40000"/></a:avLst>
          </a:prstGeom>
          <a:ln><a:tailEnd type="triangle"/></a:ln>
        </p:spPr>
        <p:style><a:lnRef idx="1"><a:schemeClr val="dk1"/></a:lnRef></p:style>
      </p:cxnSp>
    "#;

const LINE_SHAPE_XML: &str = r#"
      <p:sp xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"
            xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
        <p:nvSpPr><p:cNvPr id="43" name="Line 43"/></p:nvSpPr>
        <p:spPr>
          <a:xfrm><a:off x="952500" y="1905000"/><a:ext cx="1905000" cy="0"/></a:xfrm>
          <a:prstGeom prst="line"><a:avLst/></a:prstGeom>
          <a:ln w="12700"><a:solidFill><a:srgbClr val="FF0000"/></a:solidFill></a:ln>
        </p:spPr>
      </p:sp>
    "#;

const ELBOW_SHAPE_XML: &str = r#"
      <p:sp xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"
            xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
        <p:nvSpPr><p:cNvPr id="44" name="Elbow 44"/></p:nvSpPr>
        <p:spPr>
          <a:xfrm flipH="1"><a:off x="0" y="0"/><a:ext cx="952500" cy="1905000"/></a:xfrm>
          <a:prstGeom prst="bentConnector3">
            <a:avLst><a:gd name="adj1" fmla="val 25000"/></a:avLst>
          </a:prstGeom>
          <a:ln><a:tailEnd type="stealth"/></a:ln>
        </p:spPr>
      </p:sp>
    "#;

const GROUP_XML: &str = r#"
      <p:grpSp xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"
               xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
        <p:nvGrpSpPr><p:cNvPr id="10" name="Outer Group"/></p:nvGrpSpPr>
        <p:grpSpPr><a:xfrm>
          <a:off x="952500" y="1905000"/><a:ext cx="1905000" cy="2857500"/>
          <a:chOff x="95250" y="190500"/><a:chExt cx="952500" cy="952500"/>
        </a:xfrm></p:grpSpPr>
        <p:sp>
          <p:nvSpPr><p:cNvPr id="11" name="Panel"/></p:nvSpPr>
          <p:spPr>
            <a:xfrm><a:off x="190500" y="285750"/><a:ext cx="381000" cy="476250"/></a:xfrm>
            <a:prstGeom prst="rect"><a:avLst/></a:prstGeom>
            <a:solidFill><a:srgbClr val="FFFFFF"/></a:solidFill>
          </p:spPr>
          <p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr sz="1000"/><a:t>Panel</a:t></a:r></a:p></p:txBody>
        </p:sp>
        <p:grpSp>
          <p:nvGrpSpPr><p:cNvPr id="12" name="Inner Group"/></p:nvGrpSpPr>
          <p:grpSpPr><a:xfrm>
            <a:off x="571500" y="666750"/><a:ext cx="190500" cy="190500"/>
            <a:chOff x="0" y="0"/><a:chExt cx="95250" cy="95250"/>
          </a:xfrm></p:grpSpPr>
          <p:sp>
            <p:nvSpPr><p:cNvPr id="13" name="Nested"/></p:nvSpPr>
            <p:spPr>
              <a:xfrm><a:off x="9525" y="19050"/><a:ext cx="28575" cy="38100"/></a:xfrm>
              <a:prstGeom prst="rect"><a:avLst/></a:prstGeom>
              <a:solidFill><a:srgbClr val="FFFFFF"/></a:solidFill>
            </p:spPr>
            <p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr sz="1000"/><a:t>Nested</a:t></a:r></a:p></p:txBody>
          </p:sp>
        </p:grpSp>
      </p:grpSp>
    "#;

const BULLET_XML: &str = r#"
      <p:sp xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"
            xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
        <p:txBody><a:bodyPr/><a:lstStyle/>
          <a:p><a:pPr><a:buAutoNum type="alphaLcParenR"/></a:pPr><a:r><a:rPr sz="1000"/><a:t>First</a:t></a:r></a:p>
          <a:p><a:pPr><a:buAutoNum type="alphaLcParenR"/></a:pPr><a:r><a:rPr sz="1000"/><a:t>Second</a:t></a:r></a:p>
          <a:p><a:pPr><a:buChar char="•"/></a:pPr><a:r><a:rPr sz="1000"/><a:t>Third</a:t></a:r></a:p>
          <a:p><a:pPr><a:buAutoNum type="alphaLcParenR"/></a:pPr><a:endParaRPr sz="1000"/></a:p>
        </p:txBody>
      </p:sp>
    "#;

const LAYOUT_TEXT_STYLE_XML: &str = r#"
      <p:sldLayout xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"
                   xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
        <p:cSld><p:spTree>
        <p:sp>
          <p:nvSpPr><p:cNvPr id="1" name="Generic body"/><p:nvPr><p:ph type="body"/></p:nvPr></p:nvSpPr>
          <p:txBody><a:bodyPr anchor="ctr"/><a:lstStyle><a:lvl1pPr><a:defRPr sz="1800">
            <a:solidFill><a:schemeClr val="tx1"/></a:solidFill>
          </a:defRPr></a:lvl1pPr></a:lstStyle></p:txBody>
        </p:sp>
        <p:sp>
          <p:nvSpPr><p:cNvPr id="2" name="Eyebrow"/><p:nvPr><p:ph type="body" idx="19"/></p:nvPr></p:nvSpPr>
          <p:txBody><a:bodyPr anchor="b"/><a:lstStyle><a:lvl1pPr><a:defRPr sz="900" b="1" cap="all">
            <a:solidFill><a:schemeClr val="accent2"/></a:solidFill><a:latin typeface="+mn-lt"/>
          </a:defRPr></a:lvl1pPr></a:lstStyle></p:txBody>
        </p:sp>
        </p:spTree></p:cSld>
      </p:sldLayout>
    "#;

const MASTER_TEXT_STYLE_XML: &str = r#"
      <p:sldMaster xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"
                   xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
        <p:cSld><p:spTree><p:sp>
          <p:nvSpPr><p:cNvPr id="2" name="Title"/><p:nvPr><p:ph type="title"/></p:nvPr></p:nvSpPr>
          <p:txBody><a:bodyPr anchor="t"/><a:lstStyle/></p:txBody>
        </p:sp></p:spTree></p:cSld>
        <p:txStyles><p:titleStyle><a:lvl1pPr><a:defRPr sz="2200" b="1">
          <a:solidFill><a:schemeClr val="tx1"/></a:solidFill><a:latin typeface="+mj-lt"/>
        </a:defRPr></a:lvl1pPr></p:titleStyle></p:txStyles>
      </p:sldMaster>
    "#;

const BODY_PLACEHOLDER_XML: &str = r#"
      <p:sp xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"
            xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
        <p:nvSpPr><p:cNvPr id="20" name="Eyebrow"/><p:cNvSpPr txBox="1"/><p:nvPr><p:ph type="body" idx="19"/></p:nvPr></p:nvSpPr>
        <p:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="952500" cy="190500"/></a:xfrm><a:noFill/></p:spPr>
        <p:style><a:fontRef idx="minor"><a:schemeClr val="tx1"/></a:fontRef></p:style>
        <p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr/><a:t>Portfolio summary</a:t></a:r></a:p></p:txBody>
      </p:sp>
    "#;

const TITLE_PLACEHOLDER_XML: &str = r#"
      <p:sp xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"
            xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
        <p:nvSpPr><p:cNvPr id="21" name="Title"/><p:cNvSpPr txBox="1"/><p:nvPr><p:ph type="title"/></p:nvPr></p:nvSpPr>
        <p:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="1905000" cy="381000"/></a:xfrm><a:noFill/></p:spPr>
        <p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr sz="2000"/><a:t>Slide title</a:t></a:r></a:p></p:txBody>
      </p:sp>
    "#;

const TABLE_XML: &str = r#"
      <p:graphicFrame xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"
                      xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
        <p:xfrm><a:off x="0" y="0"/><a:ext cx="3810000" cy="1905000"/></p:xfrm>
        <a:graphic><a:graphicData><a:tbl>
          <a:tblGrid>
            <a:gridCol w="952500"/><a:gridCol w="952500"/>
            <a:gridCol w="952500"/><a:gridCol w="952500"/>
          </a:tblGrid>
          <a:tr h="952500">
            <a:tc rowSpan="2">
              <a:txBody><a:p><a:pPr algn="ctr"/><a:r><a:rPr sz="1200" b="1"><a:solidFill><a:schemeClr val="bg1"/></a:solidFill><a:latin typeface="+mn-lt"/></a:rPr><a:t>Category</a:t></a:r></a:p></a:txBody>
              <a:tcPr anchor="ctr"><a:solidFill><a:schemeClr val="tx1"/></a:solidFill></a:tcPr>
            </a:tc>
            <a:tc gridSpan="2">
              <a:txBody><a:p><a:pPr algn="ctr"/><a:r><a:rPr sz="1200"><a:solidFill><a:schemeClr val="bg1"/></a:solidFill></a:rPr><a:t>Target Position</a:t></a:r></a:p></a:txBody>
              <a:tcPr><a:solidFill><a:schemeClr val="tx1"/></a:solidFill></a:tcPr>
            </a:tc>
            <a:tc hMerge="1"><a:txBody><a:p/></a:txBody><a:tcPr/></a:tc>
            <a:tc rowSpan="2">
              <a:txBody><a:p><a:r><a:rPr sz="1200"><a:solidFill><a:schemeClr val="bg1"/></a:solidFill></a:rPr><a:t>Commentary</a:t></a:r></a:p></a:txBody>
              <a:tcPr><a:solidFill><a:schemeClr val="tx1"/></a:solidFill></a:tcPr>
            </a:tc>
          </a:tr>
          <a:tr h="0">
            <a:tc vMerge="1"><a:txBody><a:p/></a:txBody><a:tcPr/></a:tc>
            <a:tc>
              <a:txBody><a:p><a:r><a:rPr sz="1100"><a:solidFill><a:schemeClr val="tx1"/></a:solidFill></a:rPr><a:t>Value</a:t></a:r></a:p></a:txBody>
              <a:tcPr><a:solidFill><a:schemeClr val="accent3"/></a:solidFill></a:tcPr>
            </a:tc>
            <a:tc>
              <a:txBody><a:p><a:r><a:rPr sz="1100"><a:solidFill><a:schemeClr val="tx1"/></a:solidFill></a:rPr><a:t>Risk</a:t></a:r></a:p></a:txBody>
              <a:tcPr><a:solidFill><a:schemeClr val="accent3"/></a:solidFill></a:tcPr>
            </a:tc>
            <a:tc vMerge="1"><a:txBody><a:p/></a:txBody><a:tcPr/></a:tc>
          </a:tr>
        </a:tbl></a:graphicData></a:graphic>
      </p:graphicFrame>
    "#;

#[test]
fn table_import_preserves_theme_colors_fonts_and_merged_assignments() {
    let theme = ThemeColors::from_xml(THEME_XML);
    let document = Document::parse(TABLE_XML).expect("table fixture should be valid XML");
    let elements = parse_table(
        document.root_element(),
        7,
        &theme,
        &InheritedTextStyles::default(),
    );

    assert_eq!(
        elements.len(),
        5,
        "merge continuation cells are not elements"
    );
    let category = shape_with_text(&elements, "Category");
    assert_close(category.x, 0.0);
    assert_close(category.w, 100.0);
    assert_close(category.h, 200.0);
    assert_eq!(category.fill, "070154");
    assert_eq!(category.text_color, "FFFFFF");
    assert_eq!(category.font_face, "Arial");
    assert!(category.bold);
    assert_eq!(category.align, HorizontalAlign::Center);

    let target = shape_with_text(&elements, "Target Position");
    assert_close(target.x, 100.0);
    assert_close(target.w, 200.0);
    assert_close(target.h, 100.0);
    assert_eq!(target.fill, "070154");

    let commentary = shape_with_text(&elements, "Commentary");
    assert_close(commentary.x, 300.0);
    assert_close(commentary.w, 100.0);
    assert_close(commentary.h, 200.0);

    let value = shape_with_text(&elements, "Value");
    assert_close(value.x, 100.0);
    assert_close(value.y, 100.0);
    assert_eq!(value.fill, "CED7E6");
    let risk = shape_with_text(&elements, "Risk");
    assert_close(risk.x, 200.0);
    assert_close(risk.y, 100.0);
}

#[test]
fn grouped_shapes_are_recursive_and_use_the_group_coordinate_system() {
    let document = Document::parse(GROUP_XML).expect("group fixture should be valid XML");
    let parts = HashMap::new();
    let relationships = HashMap::new();
    let theme_colors = ThemeColors::default();
    let text_styles = InheritedTextStyles::default();
    let mut warnings = Vec::new();
    let mut context = SlideParseContext {
        parts: &parts,
        relationships: &relationships,
        slide_index: 0,
        source_path: "ppt/slides/slide1.xml",
        theme_colors: &theme_colors,
        text_styles: &text_styles,
        warnings: &mut warnings,
        inherited: false,
    };
    let elements =
        parse_group(&mut context, document.root_element(), 1).expect("group should import");

    assert_eq!(elements.len(), 2);
    assert!(warnings.is_empty());
    let panel = shape_with_text(&elements, "Panel");
    assert_close(panel.x, 120.0);
    assert_close(panel.y, 230.0);
    assert_close(panel.w, 80.0);
    assert_close(panel.h, 150.0);
    let nested = shape_with_text(&elements, "Nested");
    assert_close(nested.x, 204.0);
    assert_close(nested.y, 362.0);
    assert_close(nested.w, 12.0);
    assert_close(nested.h, 24.0);
}

#[test]
fn character_and_automatic_bullets_match_reference_numeric_and_empty_markers() {
    let document = Document::parse(BULLET_XML).expect("bullet fixture should be valid XML");
    let runs = text_runs(
        document.root_element(),
        &ThemeColors::default(),
        &InheritedTextStyles::default(),
    );

    assert_eq!(runs.len(), 4);
    assert_eq!(text_from_runs(&runs), "1) First\n2) Second\n• Third\n4)");
}

#[test]
fn placeholder_typography_inherits_from_layout_and_master_styles() {
    let mut parts = HashMap::new();
    parts.insert(
        "ppt/slideLayouts/slideLayout1.xml".to_owned(),
        LAYOUT_TEXT_STYLE_XML.as_bytes().to_vec(),
    );
    parts.insert(
            "ppt/slideLayouts/_rels/slideLayout1.xml.rels".to_owned(),
            br#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="../slideMasters/slideMaster1.xml"/></Relationships>"#.to_vec(),
        );
    parts.insert(
        "ppt/slideMasters/slideMaster1.xml".to_owned(),
        MASTER_TEXT_STYLE_XML.as_bytes().to_vec(),
    );
    let relationships = HashMap::from([(
        "rId1".to_owned(),
        "ppt/slideLayouts/slideLayout1.xml".to_owned(),
    )]);
    let theme = ThemeColors::from_xml(THEME_XML);
    let text_styles = InheritedTextStyles::for_slide(&parts, &relationships, &theme);

    let body_document = Document::parse(BODY_PLACEHOLDER_XML).expect("body placeholder fixture");
    let Element::Text(body) = parse_shape(body_document.root_element(), 1, &theme, &text_styles)
    else {
        panic!("body placeholder should import as text");
    };
    assert_eq!(body.text, "Portfolio summary");
    assert_eq!(body.text_color, "070154");
    assert_eq!(body.font_face, "Arial");
    // The reference chooses the first matching type when that candidate has no idx.
    assert_close(body.font_size, 18.0);
    assert!(!body.bold);
    assert_eq!(body.valign, VerticalAlign::Middle);

    let title_document = Document::parse(TITLE_PLACEHOLDER_XML).expect("title placeholder fixture");
    let Element::Text(title) = parse_shape(title_document.root_element(), 2, &theme, &text_styles)
    else {
        panic!("title placeholder should import as text");
    };
    assert_eq!(title.text, "Slide title");
    assert_eq!(title.text_color, "070154");
    assert_eq!(title.font_face, "Arial");
    assert_close(title.font_size, 20.0);
    assert!(title.bold);
    assert_eq!(title.valign, VerticalAlign::Top);
}

#[test]
fn elements_fully_beyond_the_slide_boundary_are_removed() {
    let document = Document::parse(TITLE_PLACEHOLDER_XML).expect("off-slide fixture should parse");
    let mut element = parse_shape(
        document.root_element(),
        1,
        &ThemeColors::default(),
        &InheritedTextStyles::default(),
    );
    match &mut element {
        Element::Text(text) => text.x = 1_282.0,
        _ => panic!("fixture should import as text"),
    }
    assert!(is_completely_outside_slide(&element, 1_280.0, 720.0));
    match &mut element {
        Element::Text(text) => text.x = 1_280.5,
        _ => unreachable!(),
    }
    assert!(!is_completely_outside_slide(&element, 1_280.0, 720.0));
}

#[test]
fn color_transforms_are_applied_after_theme_resolution() {
    let theme = ThemeColors::from_xml(THEME_XML);
    let document = Document::parse(
            r#"<a:solidFill xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><a:schemeClr val="dk1"><a:tint val="50000"/></a:schemeClr></a:solidFill>"#,
        )
        .expect("color fixture should be valid XML");
    assert_eq!(
        descendant_color(document.root_element(), &theme).as_deref(),
        Some("8380AA")
    );
}

#[test]
fn elbow_connector_import_preserves_geometry_and_inherited_line_style() {
    let theme = ThemeColors::from_xml(CONNECTOR_THEME_XML);
    let document =
        Document::parse(ELBOW_CONNECTOR_XML).expect("connector fixture should be valid XML");
    let line = parse_line(document.root_element(), 1, &theme);

    assert_eq!(line.line_type, LineType::Elbow);
    assert_close(line.bend_position, 0.4);
    assert_close(line.x1, 0.0);
    assert_close(line.y1, 200.0);
    assert_close(line.x2, 100.0);
    assert_close(line.y2, 0.0);
    assert_eq!(line.stroke, "070154");
    assert_close(line.stroke_width, 0.5);
    assert_close(line.stroke_opacity, 1.0);
}

#[test]
fn line_presets_stored_as_shapes_import_as_straight_and_elbow_lines() {
    let theme = ThemeColors::default();
    let straight_document =
        Document::parse(LINE_SHAPE_XML).expect("line shape fixture should be valid XML");
    let Element::Line(straight) = parse_shape(
        straight_document.root_element(),
        1,
        &theme,
        &InheritedTextStyles::default(),
    ) else {
        panic!("line preset should import as a line element");
    };
    assert_eq!(straight.line_type, LineType::Straight);
    assert_close(straight.x1, 100.0);
    assert_close(straight.y1, 200.0);
    assert_close(straight.x2, 300.0);
    assert_close(straight.y2, 200.0);
    assert_eq!(straight.stroke, "FF0000");

    let elbow_document =
        Document::parse(ELBOW_SHAPE_XML).expect("elbow shape fixture should be valid XML");
    let Element::Line(elbow) = parse_shape(
        elbow_document.root_element(),
        1,
        &theme,
        &InheritedTextStyles::default(),
    ) else {
        panic!("bent connector preset should import as a line element");
    };
    assert_eq!(elbow.line_type, LineType::Elbow);
    assert_close(elbow.bend_position, 0.25);
    assert_close(elbow.x1, 100.0);
    assert_close(elbow.x2, 0.0);
    assert_eq!(elbow.end_arrow, Some(ArrowType::Stealth));
}

#[test]
fn inherited_layout_background_and_brand_logo_are_materialized() {
    let slide_xml = r#"
            <p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
              <p:cSld><p:spTree/></p:cSld>
            </p:sld>
        "#;
    let layout_xml = r#"
            <p:sldLayout xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"
                         xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
                         xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
              <p:cSld>
                <p:bg><p:bgPr><a:blipFill><a:blip r:embed="rId2"/></a:blipFill></p:bgPr></p:bg>
                <p:spTree>
                  <p:pic>
                    <p:nvPicPr><p:cNvPr id="5" name="Picture 4" descr="West Monroe Logo"/></p:nvPicPr>
                    <p:blipFill><a:blip r:embed="rId3"/></p:blipFill>
                    <p:spPr><a:xfrm><a:off x="460377" y="6325375"/><a:ext cx="1457324" cy="305021"/></a:xfrm></p:spPr>
                  </p:pic>
                </p:spTree>
              </p:cSld>
            </p:sldLayout>
        "#;
    let layout_relationships = r#"
            <Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
              <Relationship Id="rId2"
                            Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image"
                            Target="../media/background.png"/>
              <Relationship Id="rId3"
                            Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image"
                            Target="../media/logo.png"/>
            </Relationships>
        "#;
    let mut parts = HashMap::new();
    parts.insert(
        "ppt/slideLayouts/slideLayout1.xml".to_owned(),
        layout_xml.as_bytes().to_vec(),
    );
    parts.insert(
        "ppt/slideLayouts/_rels/slideLayout1.xml.rels".to_owned(),
        layout_relationships.as_bytes().to_vec(),
    );
    parts.insert("ppt/media/background.png".to_owned(), vec![1, 2, 3]);
    parts.insert("ppt/media/logo.png".to_owned(), vec![4, 5, 6]);
    let slide_relationships = HashMap::from([(
        "rId1".to_owned(),
        "ppt/slideLayouts/slideLayout1.xml".to_owned(),
    )]);

    let theme = ThemeColors::default();
    let text_styles = InheritedTextStyles::default();
    let mut warnings = Vec::new();
    let mut context = SlideParseContext {
        parts: &parts,
        relationships: &slide_relationships,
        slide_index: 0,
        source_path: "ppt/slides/slide1.xml",
        theme_colors: &theme,
        text_styles: &text_styles,
        warnings: &mut warnings,
        inherited: false,
    };
    let elements = inherited_drawables(&mut context, slide_xml, (1280.0, 720.0)).unwrap();
    let Element::Image(image) = &elements[0] else {
        panic!("layout background image")
    };
    assert_eq!(image.alt_text, "Layout Background Image");
    assert_close(image.x, 0.0);
    assert_close(image.y, 0.0);
    assert_close(image.w, 1280.0);
    assert_close(image.h, 720.0);
    assert_eq!(image.fit, ImageFit::Stretch);
    assert_eq!(image.src, "data:image/png;base64,AQID");

    let Element::Image(logo) = &elements[1] else {
        panic!("layout brand logo")
    };
    assert_eq!(logo.alt_text, "West Monroe Logo");
    assert_close(logo.x, 48.333543307086616);
    assert_close(logo.y, 664.0813648293963);
    assert_close(logo.w, 153.0);
    assert_close(logo.h, 32.02320209973753);
    assert_eq!(logo.src, "data:image/png;base64,BAUG");
}

#[test]
fn explicit_and_paragraph_line_breaks_survive_text_import() {
    let xml = r#"
            <p:sp xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"
                  xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
              <p:txBody>
                <a:bodyPr/><a:lstStyle/>
                <a:p>
                  <a:r><a:rPr sz="1800"/><a:t>Alpha</a:t></a:r>
                  <a:br/>
                  <a:r><a:rPr sz="1800"/><a:t>Beta</a:t></a:r>
                </a:p>
                <a:p><a:r><a:rPr sz="1800"/><a:t>Gamma</a:t></a:r></a:p>
              </p:txBody>
            </p:sp>
        "#;
    let document = Document::parse(xml).expect("text fixture should be valid XML");

    let runs = text_runs(
        document.root_element(),
        &ThemeColors::default(),
        &InheritedTextStyles::default(),
    );
    let text = text_from_runs(&runs);

    assert_eq!(text, "Alpha\nBeta\nGamma");
    assert_eq!(runs.len(), 4);
    assert_eq!(runs[1].text, "\n");
    assert!(!runs[0].break_line);
    assert!(runs[2].break_line);
    assert!(runs[3].break_line);
}

fn shape_with_text<'a>(elements: &'a [Element], text: &str) -> &'a ShapeElement {
    elements
        .iter()
        .find_map(|element| match element {
            Element::Shape(shape) if shape.text == text => Some(shape),
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing table cell {text}"))
}

fn assert_close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 0.001, "{actual} != {expected}");
}
