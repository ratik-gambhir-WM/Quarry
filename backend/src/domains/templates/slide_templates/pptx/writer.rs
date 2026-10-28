use std::{fmt::Write as _, io::Write};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use zip_slide::{write::SimpleFileOptions, CompressionMethod, ZipWriter};

use super::super::{
    ArrowType, CanvasDocument, DashStyle, ElbowDirection, Element, HorizontalAlign, ImageCrop,
    ImageElement, LineElement, LineType, ShapeElement, TextElement, TextRun, VerticalAlign,
};

const EMUS_PER_PIXEL: f64 = 9_525.0;
const EMUS_PER_INCH: f64 = 914_400.0;

pub(super) fn write(document: &CanvasDocument) -> anyhow::Result<Vec<u8>> {
    anyhow::ensure!(
        !document.presentation.slides.is_empty(),
        "presentation has no slides"
    );
    let cursor = super::output::BoundedCursor::new();
    let mut zip = ZipWriter::new(cursor);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    add_xml(
        &mut zip,
        "[Content_Types].xml",
        &content_types(document),
        options,
    )?;
    add_xml(&mut zip, "_rels/.rels", ROOT_RELS, options)?;
    add_xml(
        &mut zip,
        "docProps/app.xml",
        &app_properties(document.presentation.slides.len()),
        options,
    )?;
    add_xml(
        &mut zip,
        "docProps/core.xml",
        &core_properties(&document.presentation.title),
        options,
    )?;
    add_xml(
        &mut zip,
        "ppt/presentation.xml",
        &presentation_xml(document),
        options,
    )?;
    add_xml(
        &mut zip,
        "ppt/_rels/presentation.xml.rels",
        &presentation_rels(document),
        options,
    )?;
    add_xml(
        &mut zip,
        "ppt/slideMasters/slideMaster1.xml",
        SLIDE_MASTER,
        options,
    )?;
    add_xml(
        &mut zip,
        "ppt/slideMasters/_rels/slideMaster1.xml.rels",
        SLIDE_MASTER_RELS,
        options,
    )?;
    let mut layout = SLIDE_LAYOUT.to_owned();
    let mut layout_rels = SLIDE_LAYOUT_RELS.to_owned();
    if document.presentation.show_branding {
        let mut branding_shapes = String::new();
        let mut branding_media = Vec::new();
        write_brand_frame(
            &mut branding_shapes,
            2,
            &document.presentation.slides[0],
            &mut branding_media,
        )?;
        layout = layout.replace(
            "<p:grpSpPr/></p:spTree>",
            &format!("<p:grpSpPr/>{branding_shapes}</p:spTree>"),
        );
        for item in branding_media {
            layout_rels = layout_rels.replace("</Relationships>", &format!(r#"<Relationship Id="rId{}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="../media/{}"/></Relationships>"#, item.relationship_id, item.file_name));
            zip.start_file(format!("ppt/media/{}", item.file_name), options)?;
            zip.write_all(&item.bytes)?;
        }
    }
    add_xml(
        &mut zip,
        "ppt/slideLayouts/slideLayout1.xml",
        &layout,
        options,
    )?;
    add_xml(
        &mut zip,
        "ppt/slideLayouts/_rels/slideLayout1.xml.rels",
        &layout_rels,
        options,
    )?;
    add_xml(&mut zip, "ppt/theme/theme1.xml", THEME, options)?;
    add_xml(&mut zip, "ppt/presProps.xml", PRES_PROPS, options)?;
    add_xml(&mut zip, "ppt/viewProps.xml", VIEW_PROPS, options)?;
    add_xml(&mut zip, "ppt/tableStyles.xml", TABLE_STYLES, options)?;

    for (slide_index, slide) in document.presentation.slides.iter().enumerate() {
        let mut media = Vec::new();
        let xml = slide_xml(
            slide,
            document.presentation.preserve_element_order,
            slide_index,
            &mut media,
        )?;
        add_xml(
            &mut zip,
            &format!("ppt/slides/slide{}.xml", slide_index + 1),
            &xml,
            options,
        )?;
        add_xml(
            &mut zip,
            &format!("ppt/slides/_rels/slide{}.xml.rels", slide_index + 1),
            &slide_rels(&media),
            options,
        )?;
        for item in media {
            zip.start_file(format!("ppt/media/{}", item.file_name), options)?;
            zip.write_all(&item.bytes)?;
        }
    }

    Ok(zip.finish()?.into_inner())
}

fn add_xml(
    zip: &mut ZipWriter<super::output::BoundedCursor>,
    path: &str,
    xml: &str,
    options: SimpleFileOptions,
) -> anyhow::Result<()> {
    zip.start_file(path, options)?;
    zip.write_all(xml.as_bytes())?;
    Ok(())
}

struct Media {
    relationship_id: usize,
    file_name: String,
    bytes: Vec<u8>,
}

fn slide_xml(
    slide: &super::super::Slide,
    preserve_element_order: bool,
    slide_index: usize,
    media: &mut Vec<Media>,
) -> anyhow::Result<String> {
    let mut shapes = String::new();
    let mut shape_id = 2_u32;
    let mut elements = slide.elements.iter().collect::<Vec<_>>();
    if !preserve_element_order {
        elements.sort_by_key(|element| element_layer(element, slide));
    }
    for element in elements {
        match element {
            Element::Text(element) => write_text(&mut shapes, shape_id, element),
            Element::Shape(element) => write_shape(&mut shapes, shape_id, element),
            Element::Line(element) => write_line(&mut shapes, shape_id, element),
            Element::Image(element) => {
                let (content_type, bytes) = decode_image(&element.src)?;
                let extension = extension(&content_type);
                let relationship_id = media.len() + 2;
                media.push(Media {
                    relationship_id,
                    file_name: format!(
                        "slide-{}-image-{}.{}",
                        slide_index + 1,
                        media.len() + 1,
                        extension
                    ),
                    bytes,
                });
                write_image(&mut shapes, shape_id, relationship_id, element);
            }
        }
        shape_id = shape_id.saturating_add(1);
    }
    Ok(format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:cSld name="{}"><p:bg><p:bgPr><a:solidFill><a:srgbClr val="{}"/></a:solidFill><a:effectLst/></p:bgPr></p:bg><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr>{}</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sld>"#,
        xml_escape(&slide.name),
        safe_color(&slide.background_color, "FFFFFF"),
        shapes
    ))
}

fn element_layer(element: &Element, slide: &super::super::Slide) -> u8 {
    let id = match element {
        Element::Text(value) => &value.id,
        Element::Shape(value) => &value.id,
        Element::Line(value) => &value.id,
        Element::Image(value) => &value.id,
    };
    if id == "west-monroe-footer" || id.starts_with("west-monroe-dot-") {
        return 0;
    }
    match element {
        Element::Shape(shape)
            if shape.shape == "rect"
                && (shape.w * shape.h / (slide.width * slide.height).max(1.0) >= 0.08
                    || shape.h / slide.height >= 0.45 && shape.w / slide.width >= 0.12) =>
        {
            10
        }
        Element::Line(_) => 20,
        _ => 30,
    }
}

fn rounded_adjustment(shape: &str, width: f64, height: f64, radius: f64) -> String {
    if shape == "roundRect" && radius > 0.0 && width.min(height) > 0.0 {
        format!(
            r#"<a:avLst><a:gd name="adj" fmla="val {}"/></a:avLst>"#,
            (radius / width.min(height) * 100_000.0)
                .clamp(0.0, 50_000.0)
                .round() as i64
        )
    } else {
        "<a:avLst/>".to_owned()
    }
}

fn write_brand_frame(
    output: &mut String,
    mut shape_id: u32,
    slide: &super::super::Slide,
    media: &mut Vec<Media>,
) -> anyhow::Result<u32> {
    const BRAND_COLOR: &str = "E8EEF8";
    let scale_x = slide.width / 1280.0;
    let scale_y = slide.height / 720.0;
    write_brand_rect(
        output,
        shape_id,
        0.0,
        slide.height - 75.0 * scale_y,
        slide.width,
        75.0 * scale_y,
        BRAND_COLOR,
    );
    shape_id = shape_id.saturating_add(1);
    for index in 0..(15 * 17) {
        write_brand_rect(
            output,
            shape_id,
            (15.0 + f64::from(index % 15) * 19.0) * scale_x,
            (6.0 + f64::from(index / 15) * 19.0) * scale_y,
            2.5 * scale_x,
            2.5 * scale_y,
            BRAND_COLOR,
        );
        shape_id = shape_id.saturating_add(1);
    }
    let relationship_id = media.len() + 2;
    media.push(Media {
        relationship_id,
        file_name: format!("slide-{}-brand-logo.png", xml_name(&slide.id)),
        bytes: STANDARD.decode(include_str!("assets/west_monroe_logo.b64").trim())?,
    });
    write_image(
        output,
        shape_id,
        relationship_id,
        &ImageElement {
            id: "west-monroe-logo".to_owned(),
            x: 48.33 * scale_x,
            y: 664.08 * scale_y,
            w: 153.0 * scale_x,
            h: 32.02 * scale_y,
            src: String::new(),
            fit: super::super::ImageFit::Stretch,
            crop: None,
            alt_text: "West Monroe".to_owned(),
            opacity: 1.0,
            rotate: 0.0,
            flip_h: false,
            flip_v: false,
            border_radius: 0.0,
        },
    );
    Ok(shape_id.saturating_add(1))
}

fn write_brand_rect(output: &mut String, id: u32, x: f64, y: f64, w: f64, h: f64, color: &str) {
    write!(
        output,
        r#"<p:sp><p:nvSpPr><p:cNvPr id="{}" name="brand-frame-{}"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr>{}<a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:solidFill><a:srgbClr val="{}"/></a:solidFill><a:ln><a:noFill/></a:ln></p:spPr></p:sp>"#,
        id,
        id,
        transform(x, y, w, h, 0.0, false, false),
        color
    )
    .ok();
}

fn write_text(output: &mut String, id: u32, element: &TextElement) {
    let shape = if element.border_radius > 0.0 {
        "roundRect"
    } else {
        "rect"
    };
    write!(output, r#"<p:sp><p:nvSpPr><p:cNvPr id="{}" name="{}"/><p:cNvSpPr txBox="1"/><p:nvPr/></p:nvSpPr><p:spPr>{}<a:prstGeom prst="{}">{}</a:prstGeom>{}{}</p:spPr>{}</p:sp>"#,
        id, xml_escape(&element.id), transform(element.x, element.y, element.w, element.h, element.rotate, element.flip_h, element.flip_v), shape, rounded_adjustment(shape, element.w, element.h, element.border_radius),
        fill_xml(&element.fill, element.fill_opacity * element.opacity), line_xml(&element.stroke, element.stroke_width, element.stroke_opacity * element.opacity),
        text_body(&element.text, &element.runs, TextBodyOptions {
            font_face: &element.font_face, font_size: element.font_size,
            color: &element.text_color, bold: element.bold, italic: element.italic,
            padding: element.padding, align: element.align, valign: element.valign,
        })
    ).ok();
}

fn write_shape(output: &mut String, id: u32, element: &ShapeElement) {
    let shape = if element.shape == "rect"
        && !element.text.trim().is_empty()
        && element.border_radius > 0.0
    {
        "roundRect"
    } else {
        pptx_shape(&element.shape)
    };
    write!(output, r#"<p:sp><p:nvSpPr><p:cNvPr id="{}" name="{}"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr>{}<a:prstGeom prst="{}">{}</a:prstGeom>{}{}</p:spPr>{}</p:sp>"#,
        id, xml_escape(&element.id), transform(element.x, element.y, element.w, element.h, element.rotate, element.flip_h, element.flip_v), shape, rounded_adjustment(shape, element.w, element.h, element.border_radius),
        fill_xml(&element.fill, element.fill_opacity * element.opacity), line_xml(&element.stroke, element.stroke_width, element.stroke_opacity * element.opacity),
        if element.text.trim().is_empty() { String::new() } else { text_body(&element.text, &element.runs, TextBodyOptions {
            font_face: &element.font_face, font_size: element.font_size,
            color: &element.text_color, bold: element.bold, italic: false,
            padding: element.padding, align: element.align, valign: element.valign,
        }) }
    ).ok();
}

fn write_line(output: &mut String, id: u32, element: &LineElement) {
    let is_elbow = element.line_type == LineType::Elbow
        && (element.x2 - element.x1).abs() > 0.5
        && (element.y2 - element.y1).abs() > 0.5;
    let reverse = is_elbow && element.elbow_direction == ElbowDirection::VerticalFirst;
    let (x1, y1, x2, y2, begin_arrow, end_arrow) = if reverse {
        (
            element.x2,
            element.y2,
            element.x1,
            element.y1,
            element.end_arrow,
            element.begin_arrow,
        )
    } else {
        (
            element.x1,
            element.y1,
            element.x2,
            element.y2,
            element.begin_arrow,
            element.end_arrow,
        )
    };
    let x = x1.min(x2);
    let y = y1.min(y2);
    let w = (x2 - x1).abs();
    let h = (y2 - y1).abs();
    let flip_h = x2 < x1;
    let flip_v = y2 < y1;
    let dash = match element.dash {
        DashStyle::Solid => "solid",
        DashStyle::Dash => "dash",
        DashStyle::Dot => "sysDot",
    };
    let preset = if is_elbow { "bentConnector2" } else { "line" };
    let adjustments = "<a:avLst/>";
    write!(output, r#"<p:cxnSp><p:nvCxnSpPr><p:cNvPr id="{}" name="{}"/><p:cNvCxnSpPr/><p:nvPr/></p:nvCxnSpPr><p:spPr>{}<a:prstGeom prst="{}">{}</a:prstGeom><a:ln w="{}"><a:solidFill><a:srgbClr val="{}"><a:alpha val="{}"/></a:srgbClr></a:solidFill><a:prstDash val="{}"/>{}{}</a:ln></p:spPr></p:cxnSp>"#,
        id, xml_escape(&element.id), transform(x, y, w, h, element.rotate, flip_h, flip_v), preset, adjustments, points_to_emu(element.stroke_width), safe_color(&element.stroke, "334155"), alpha(element.stroke_opacity * element.opacity), dash,
        arrow_xml("headEnd", begin_arrow), arrow_xml("tailEnd", end_arrow)
    ).ok();
}

fn write_image(output: &mut String, id: u32, relationship_id: usize, element: &ImageElement) {
    let crop = element.crop.map(crop_xml).unwrap_or_default();
    write!(output, r#"<p:pic><p:nvPicPr><p:cNvPr id="{}" name="{}" descr="{}"/><p:cNvPicPr><a:picLocks noChangeAspect="1"/></p:cNvPicPr><p:nvPr/></p:nvPicPr><p:blipFill><a:blip r:embed="rId{}"><a:alphaModFix amt="{}"/></a:blip>{}<a:stretch><a:fillRect/></a:stretch></p:blipFill><p:spPr>{}<a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:noFill/><a:ln><a:noFill/></a:ln></p:spPr></p:pic>"#,
        id, xml_escape(&element.id), xml_escape(&element.alt_text), relationship_id, alpha(element.opacity), crop,
        transform(element.x, element.y, element.w, element.h, element.rotate, element.flip_h, element.flip_v)
    ).ok();
}

struct TextBodyOptions<'a> {
    font_face: &'a str,
    font_size: f64,
    color: &'a str,
    bold: bool,
    italic: bool,
    padding: f64,
    align: HorizontalAlign,
    valign: VerticalAlign,
}

fn text_body(text: &str, runs: &[TextRun], options: TextBodyOptions<'_>) -> String {
    let align = match options.align {
        HorizontalAlign::Left => "l",
        HorizontalAlign::Center => "ctr",
        HorizontalAlign::Right => "r",
    };
    let anchor = match options.valign {
        VerticalAlign::Top => "t",
        VerticalAlign::Middle => "ctr",
        VerticalAlign::Bottom => "b",
    };
    let margin = points_to_emu(options.padding);
    let effective_runs = if runs.is_empty() {
        vec![TextRun {
            text: text.to_owned(),
            color: options.color.to_owned(),
            font_face: options.font_face.to_owned(),
            font_size: options.font_size,
            bold: options.bold,
            italic: options.italic,
            underline: false,
            break_line: false,
        }]
    } else {
        runs.to_vec()
    };
    let mut body = format!(
        r#"<p:txBody><a:bodyPr wrap="square" lIns="{margin}" tIns="{margin}" rIns="{margin}" bIns="{margin}" anchor="{anchor}"><a:normAutofit/></a:bodyPr><a:lstStyle/><a:p><a:pPr algn="{align}"><a:lnSpc><a:spcPct val="100000"/></a:lnSpc><a:spcBef><a:spcPts val="0"/></a:spcBef><a:spcAft><a:spcPts val="0"/></a:spcAft></a:pPr>"#
    );
    for (index, run) in effective_runs.iter().enumerate() {
        for (line_index, line) in run.text.split('\n').enumerate() {
            if line_index > 0 {
                write!(body, r#"</a:p><a:p><a:pPr algn="{align}"/>"#).ok();
            }
            write!(body, r#"<a:r><a:rPr lang="en-US" sz="{}" b="{}" i="{}" u="{}"><a:solidFill><a:srgbClr val="{}"/></a:solidFill><a:latin typeface="{}"/></a:rPr><a:t>{}</a:t></a:r>"#,
            (run.font_size * 100.0).round() as i64, u8::from(run.bold), u8::from(run.italic), if run.underline { "sng" } else { "none" }, safe_color(&run.color, options.color), xml_escape(&run.font_face), xml_escape(line)
        ).ok();
        }
        if run.break_line && index + 1 < effective_runs.len() {
            write!(body, r#"</a:p><a:p><a:pPr algn="{align}"/>"#).ok();
        }
    }
    write!(
        body,
        r#"<a:endParaRPr lang="en-US" sz="{}"/></a:p></p:txBody>"#,
        (options.font_size * 100.0).round() as i64
    )
    .ok();
    body
}

fn transform(x: f64, y: f64, w: f64, h: f64, rotate: f64, flip_h: bool, flip_v: bool) -> String {
    let rotation = (rotate * 60_000.0).round() as i64;
    format!(
        r#"<a:xfrm rot="{rotation}" flipH="{}" flipV="{}"><a:off x="{}" y="{}"/><a:ext cx="{}" cy="{}"/></a:xfrm>"#,
        u8::from(flip_h),
        u8::from(flip_v),
        px_to_emu(x),
        px_to_emu(y),
        px_to_emu(w.max(0.0)),
        px_to_emu(h.max(0.0))
    )
}

fn fill_xml(color: &str, opacity: f64) -> String {
    if color.eq_ignore_ascii_case("transparent") {
        return r#"<a:solidFill><a:srgbClr val="FFFFFF"><a:alpha val="0"/></a:srgbClr></a:solidFill>"#.to_owned();
    }
    format!(
        r#"<a:solidFill><a:srgbClr val="{}"><a:alpha val="{}"/></a:srgbClr></a:solidFill>"#,
        safe_color(color, "FFFFFF"),
        alpha(opacity)
    )
}

fn line_xml(color: &str, width: f64, opacity: f64) -> String {
    if color.eq_ignore_ascii_case("transparent") || width <= 0.0 {
        return r#"<a:ln w="0"><a:solidFill><a:srgbClr val="FFFFFF"><a:alpha val="0"/></a:srgbClr></a:solidFill></a:ln>"#.to_owned();
    }
    format!(
        r#"<a:ln w="{}"><a:solidFill><a:srgbClr val="{}"><a:alpha val="{}"/></a:srgbClr></a:solidFill></a:ln>"#,
        points_to_emu(width),
        safe_color(color, "000000"),
        alpha(opacity)
    )
}

fn arrow_xml(tag: &str, arrow: Option<ArrowType>) -> String {
    let value = match arrow {
        Some(ArrowType::Triangle) => "triangle",
        Some(ArrowType::Arrow) => "arrow",
        Some(ArrowType::Diamond) => "diamond",
        Some(ArrowType::Oval) => "oval",
        Some(ArrowType::Stealth) => "stealth",
        None => "none",
    };
    format!("<a:{tag} type=\"{value}\"/>")
}

fn crop_xml(crop: ImageCrop) -> String {
    format!(
        r#"<a:srcRect t="{}" r="{}" b="{}" l="{}"/>"#,
        ratio(crop.top),
        ratio(crop.right),
        ratio(crop.bottom),
        ratio(crop.left)
    )
}

fn decode_image(source: &str) -> anyhow::Result<(String, Vec<u8>)> {
    let rest = source
        .strip_prefix("data:")
        .ok_or_else(|| anyhow::anyhow!("image is not embedded"))?;
    let (content_type, encoded) = rest
        .split_once(";base64,")
        .ok_or_else(|| anyhow::anyhow!("image is not base64 encoded"))?;
    Ok((content_type.to_ascii_lowercase(), STANDARD.decode(encoded)?))
}

fn extension(content_type: &str) -> &'static str {
    match content_type {
        "image/png" => "png",
        "image/jpeg" | "image/jpg" => "jpg",
        "image/gif" => "gif",
        "image/svg+xml" => "svg",
        "image/emf" => "emf",
        "image/webp" => "webp",
        "image/bmp" => "bmp",
        "image/tiff" => "tiff",
        "image/wmf" => "wmf",
        _ => "bin",
    }
}

fn safe_color(value: &str, fallback: &str) -> String {
    let value = value.trim_start_matches('#');
    if value.len() >= 6 && value.as_bytes()[..6].iter().all(u8::is_ascii_hexdigit) {
        value[..6].to_ascii_uppercase()
    } else {
        fallback.to_owned()
    }
}

fn pptx_shape(value: &str) -> &str {
    match value.to_ascii_lowercase().as_str() {
        "rectangle" => "rect",
        "roundedrectangle" | "roundrect" => "roundRect",
        "circle" | "oval" => "ellipse",
        "database" | "cylinder" | "flowchartmagneticdisk" => "flowChartMagneticDisk",
        "lineinv" => "lineInv",
        _ => value,
    }
}

fn px_to_emu(value: f64) -> i64 {
    (value * EMUS_PER_PIXEL).round() as i64
}
fn points_to_emu(value: f64) -> i64 {
    (value * 12_700.0).round() as i64
}
fn alpha(value: f64) -> i64 {
    // PptxGenJS accepts an integer percentage of transparency.
    ((1.0 - ((1.0 - value.clamp(0.0, 1.0)) * 100.0).round() / 100.0) * 100_000.0).round() as i64
}
fn ratio(value: f64) -> i64 {
    (value.clamp(0.0, 1.0) * 100_000.0).round() as i64
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn xml_name(value: &str) -> String {
    let name = value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || *ch == '-')
        .collect::<String>();
    if name.is_empty() {
        "slide".to_owned()
    } else {
        name
    }
}

fn content_types(document: &CanvasDocument) -> String {
    let mut defaults = String::from(
        r#"<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/>"#,
    );
    let mut seen = std::collections::BTreeSet::new();
    if document.presentation.show_branding {
        defaults.push_str(r#"<Default Extension="png" ContentType="image/png"/>"#);
        seen.insert("png");
    }
    for image in document
        .presentation
        .slides
        .iter()
        .flat_map(|slide| &slide.elements)
        .filter_map(|element| {
            if let Element::Image(image) = element {
                Some(image)
            } else {
                None
            }
        })
    {
        if let Ok((content_type, _)) = decode_image(&image.src) {
            let extension = extension(&content_type);
            if seen.insert(extension) {
                write!(
                    defaults,
                    r#"<Default Extension="{}" ContentType="{}"/>"#,
                    extension, content_type
                )
                .ok();
            }
        }
    }
    let mut overrides = String::from(
        r#"<Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/><Override PartName="/ppt/slideMasters/slideMaster1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml"/><Override PartName="/ppt/slideLayouts/slideLayout1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml"/><Override PartName="/ppt/theme/theme1.xml" ContentType="application/vnd.openxmlformats-officedocument.theme+xml"/><Override PartName="/ppt/presProps.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presProps+xml"/><Override PartName="/ppt/viewProps.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.viewProps+xml"/><Override PartName="/ppt/tableStyles.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.tableStyles+xml"/><Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/><Override PartName="/docProps/app.xml" ContentType="application/vnd.openxmlformats-officedocument.extended-properties+xml"/>"#,
    );
    for index in 1..=document.presentation.slides.len() {
        write!(overrides, r#"<Override PartName="/ppt/slides/slide{}.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/>"#, index).ok();
    }
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">{defaults}{overrides}</Types>"#
    )
}

fn presentation_xml(document: &CanvasDocument) -> String {
    let width = document
        .presentation
        .slides
        .first()
        .map_or(1280.0, |slide| slide.width);
    let height = document
        .presentation
        .slides
        .first()
        .map_or(720.0, |slide| slide.height);
    let ids = (0..document.presentation.slides.len())
        .map(|index| format!(r#"<p:sldId id="{}" r:id="rId{}"/>"#, 256 + index, index + 2))
        .collect::<String>();
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><p:presentation xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:sldMasterIdLst><p:sldMasterId id="2147483648" r:id="rId1"/></p:sldMasterIdLst><p:sldIdLst>{ids}</p:sldIdLst><p:sldSz cx="{}" cy="{}" type="custom"/><p:notesSz cx="6858000" cy="9144000"/><p:defaultTextStyle/></p:presentation>"#,
        (width / 96.0 * EMUS_PER_INCH).round() as i64,
        (height / 96.0 * EMUS_PER_INCH).round() as i64
    )
}

fn presentation_rels(document: &CanvasDocument) -> String {
    let mut relationships = String::from(
        r#"<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="slideMasters/slideMaster1.xml"/>"#,
    );
    for index in 0..document.presentation.slides.len() {
        write!(relationships, r#"<Relationship Id="rId{}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide{}.xml"/>"#, index + 2, index + 1).ok();
    }
    let base = document.presentation.slides.len() + 2;
    write!(relationships, r#"<Relationship Id="rId{}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/presProps" Target="presProps.xml"/><Relationship Id="rId{}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/viewProps" Target="viewProps.xml"/><Relationship Id="rId{}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/tableStyles" Target="tableStyles.xml"/>"#, base, base + 1, base + 2).ok();
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">{relationships}</Relationships>"#
    )
}

fn slide_rels(media: &[Media]) -> String {
    let mut relationships = String::from(
        r#"<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/>"#,
    );
    for item in media {
        write!(relationships, r#"<Relationship Id="rId{}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="../media/{}"/>"#, item.relationship_id, item.file_name).ok();
    }
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">{relationships}</Relationships>"#
    )
}

fn app_properties(slides: usize) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties" xmlns:vt="http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes"><Application>slide-template-endpoints</Application><PresentationFormat>Custom</PresentationFormat><Slides>{slides}</Slides><Notes>0</Notes><HiddenSlides>0</HiddenSlides><MMClips>0</MMClips><ScaleCrop>false</ScaleCrop><Company>OpenAI</Company><LinksUpToDate>false</LinksUpToDate><SharedDoc>false</SharedDoc><HyperlinksChanged>false</HyperlinksChanged><AppVersion>1.0</AppVersion></Properties>"#
    )
}
fn core_properties(title: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/" xmlns:dcmitype="http://purl.org/dc/dcmitype/" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"><dc:title>{}</dc:title><dc:creator>OpenAI Codex</dc:creator><cp:lastModifiedBy>OpenAI Codex</cp:lastModifiedBy></cp:coreProperties>"#,
        xml_escape(title)
    )
}

const ROOT_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="ppt/presentation.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/><Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties" Target="docProps/app.xml"/></Relationships>"#;
const SLIDE_MASTER: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><p:sldMaster xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/></p:spTree></p:cSld><p:clrMap accent1="accent1" accent2="accent2" accent3="accent3" accent4="accent4" accent5="accent5" accent6="accent6" bg1="lt1" bg2="lt2" folHlink="folHlink" hlink="hlink" tx1="dk1" tx2="dk2"/><p:sldLayoutIdLst><p:sldLayoutId id="2147483649" r:id="rId1"/></p:sldLayoutIdLst><p:txStyles><p:titleStyle/><p:bodyStyle/><p:otherStyle/></p:txStyles></p:sldMaster>"#;
const SLIDE_MASTER_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" Target="../theme/theme1.xml"/></Relationships>"#;
const SLIDE_LAYOUT: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><p:sldLayout xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" type="blank" preserve="1"><p:cSld name="Blank"><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/></p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sldLayout>"#;
const SLIDE_LAYOUT_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="../slideMasters/slideMaster1.xml"/></Relationships>"#;
// Copied from the PptxGenJS package emitted by the working tts-mermaid exporter,
// then themed with that exporter's unchanged PowerpointTheme transformation.
const THEME: &str = include_str!("assets/theme1.xml");
const PRES_PROPS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><p:presentationPr xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"/>"#;
// PptxGenJS supplies the required nested view records; empty records make
// PowerPoint repair and remove the package during open.
const VIEW_PROPS: &str = include_str!("assets/viewProps.xml");
const TABLE_STYLES: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><a:tblStyleLst xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" def="{5C22544A-7EE6-4342-B048-85BDC9FD1C3A}"/>"#;
