//! Academic templates: papers laid out the way common style guides ask (APA 7, IEEE, ABNT,
//! Chicago, and the author guidelines journals publish), each with its citation style set.
//!
//! These are WordCraft's own documents built from the public rules (margins, fonts, spacing,
//! heading styles, section order). They are not the publishers' templates and carry none of their
//! files, logos or wording; check the journal's current instructions before submitting.

use std::sync::Arc;

use wordcraft_doc::props::{Align, CharProps, LineSpacing, TextColor};
use wordcraft_doc::section::{Columns, SectionStart};
use wordcraft_doc::{Block, Document, Paragraph};

/// The academic templates: (id, name).
pub const LIST: [(&str, &str); 9] = [
    ("acad-apa7", "APA 7 Student Paper"),
    ("acad-ieee", "IEEE-style Conference Paper"),
    ("acad-abnt", "ABNT Article (NBR 6022)"),
    ("acad-chicago", "Chicago Paper"),
    ("acad-nature", "Nature-style Letter"),
    ("acad-elsevier", "Elsevier-style Article"),
    ("acad-springer", "Springer-style Article"),
    ("acad-thesis", "Thesis (ABNT NBR 14724)"),
    ("acad-response", "Response to Reviewers"),
];

/// The citation style a template uses.
pub fn style_of(id: &str) -> Option<&'static str> {
    Some(match id {
        "acad-apa7" => "APA",
        "acad-ieee" => "IEEE",
        "acad-abnt" | "acad-thesis" => "ISO 690 (author-date)",
        "acad-chicago" => "Chicago",
        "acad-nature" => "ISO 690 (numerical)",
        "acad-elsevier" => "Harvard (Anglia)",
        "acad-springer" => "APA",
        _ => return None,
    })
}

/// How a template's text is set.
struct Look {
    font: &'static str,
    size: f32,
    line: f32,
    /// Page margins (top, right, bottom, left), points.
    margins: [f32; 4],
    /// A4 rather than US Letter.
    a4: bool,
    indent_first: f32,
    justify: bool,
}

fn chr(size: f32, bold: bool, italic: bool) -> CharProps {
    CharProps { size: Some(size), bold: bold.then_some(true), italic: italic.then_some(true), ..Default::default() }
}

fn para(text: &str, props: CharProps) -> Paragraph {
    Paragraph::with_text(text, props)
}

fn plain(text: &str) -> Paragraph {
    Paragraph::with_text(text, CharProps::default())
}

fn styled(text: &str, style: &str) -> Paragraph {
    plain(text).styled(style)
}

fn center(mut p: Paragraph) -> Paragraph {
    p.props.align = Some(Align::Center);
    p.props.indent_first = Some(0.0);
    p
}

fn flush(mut p: Paragraph) -> Paragraph {
    p.props.indent_first = Some(0.0);
    p
}

fn page_break_before(mut p: Paragraph) -> Paragraph {
    p.props.page_break_before = Some(true);
    p
}

fn grey(mut p: Paragraph) -> Paragraph {
    for r in &mut p.runs {
        r.props.color = Some(TextColor::Rgb(wordcraft_doc::props::Rgb(0x6B, 0x6B, 0x6B)));
    }
    p
}

/// A document from paragraphs, with its body text and headings set by `look`.
fn build(paras: Vec<Paragraph>, look: &Look, headings: &[(&str, f32, bool, bool, Align)]) -> Document {
    let mut d = Document::new();
    // A line break in the text starts a new paragraph set the same way.
    let mut out: Vec<Paragraph> = Vec::new();
    for p in paras {
        if !p.text.contains('\n') {
            out.push(p);
            continue;
        }
        let props = p.runs.first().map(|r| r.props.clone()).unwrap_or_default();
        let lines: Vec<&str> = p.text.split('\n').collect();
        let last = lines.len().saturating_sub(1);
        for (k, line) in lines.iter().enumerate() {
            let mut q = Paragraph::with_text(line, props.clone());
            q.props = p.props.clone();
            if k > 0 {
                q.props.space_before = Some(0.0);
                q.props.page_break_before = None;
            }
            if k < last {
                q.props.space_after = Some(0.0);
            }
            out.push(q);
        }
    }
    d.body = out.into_iter().map(|p| Arc::new(Block::Para(p))).collect();
    let s = &mut d.last_section;
    if look.a4 {
        s.page_w = 595.3;
        s.page_h = 841.9;
    } else {
        s.page_w = 612.0;
        s.page_h = 792.0;
    }
    let [t, r, b, l] = look.margins;
    s.margin_top = t;
    s.margin_right = r;
    s.margin_bottom = b;
    s.margin_left = l;
    if let Some(mut normal) = d.styles.get("Normal").cloned() {
        normal.chr.font = Some(look.font.into());
        normal.chr.size = Some(look.size);
        normal.para.line_spacing = Some(LineSpacing::Multiple(look.line));
        normal.para.space_after = Some(0.0);
        normal.para.space_before = Some(0.0);
        normal.para.indent_first = Some(look.indent_first);
        if look.justify {
            normal.para.align = Some(Align::Justify);
        }
        d.styles.upsert(normal);
    }
    for (id, size, bold, italic, align) in headings {
        if let Some(mut h) = d.styles.get(id).cloned() {
            h.chr.font = Some(look.font.into());
            h.chr.size = Some(*size);
            h.chr.bold = Some(*bold);
            h.chr.italic = Some(*italic);
            h.chr.color = Some(TextColor::Auto);
            h.para.align = Some(*align);
            h.para.indent_first = Some(0.0);
            h.para.space_before = Some(look.size);
            h.para.space_after = Some(look.size * 0.5);
            d.styles.upsert(h);
        }
    }
    d
}

/// A template's document.
pub fn document(id: &str) -> Option<Document> {
    let mut d = match id {
        "acad-apa7" => apa7(),
        "acad-ieee" => ieee(),
        "acad-abnt" => abnt(),
        "acad-chicago" => chicago(),
        "acad-nature" => nature(),
        "acad-elsevier" => elsevier(),
        "acad-springer" => springer(),
        "acad-thesis" => thesis(),
        "acad-response" => response_letter_template(),
        _ => return None,
    };
    d.settings.bib_style = style_of(id).map(str::to_string);
    Some(d)
}

const INCH: f32 = 72.0;

fn apa7() -> Document {
    let look = Look { font: "Times New Roman", size: 12.0, line: 2.0, margins: [INCH; 4], a4: false, indent_first: 36.0, justify: false };
    let paras = vec![
        center(para("", chr(12.0, false, false))),
        center(para("", chr(12.0, false, false))),
        center(para("Title of the Paper in Title Case", chr(12.0, true, false))),
        center(plain("")),
        center(plain("Student Name")),
        center(plain("Department, University")),
        center(plain("Course Number: Course Name")),
        center(plain("Instructor Name")),
        center(plain("Due Date")),
        page_break_before(center(para("Title of the Paper in Title Case", chr(12.0, true, false)))),
        plain(
            "Open with the problem and why it matters. APA 7 sets everything in one font, double-spaced, with a half-inch indent on each paragraph. Cite sources in the text as (Author, Year).",
        ),
        styled("Method", "Heading1"),
        styled("Participants", "Heading2"),
        plain("Who took part, how they were recruited, and how many."),
        styled("Materials and Procedure", "Heading2"),
        plain("What was used and what happened, in order."),
        styled("Results", "Heading1"),
        plain("Report statistics in APA form, for example t(28) = 2.45, p = .021."),
        styled("Discussion", "Heading1"),
        plain("What the results mean, their limitations, and what comes next."),
        page_break_before(center(para("References", chr(12.0, true, false)))),
        flush(plain("Insert the bibliography here (Research › Bibliography). APA lists references alphabetically with a hanging indent.")),
    ];
    build(
        paras,
        &look,
        &[("Heading1", 12.0, true, false, Align::Center), ("Heading2", 12.0, true, false, Align::Left), ("Heading3", 12.0, true, true, Align::Left)],
    )
}

fn ieee() -> Document {
    let look =
        Look { font: "Times New Roman", size: 10.0, line: 1.0, margins: [54.0, 44.0, 72.0, 44.0], a4: false, indent_first: 10.0, justify: true };
    let mut author = center(plain("First Author, Second Author\nDept. of Engineering, University, City, Country\nemail@example.org"));
    author.props.space_after = Some(14.0);
    let last_of_title = center(para("", chr(10.0, false, false)));
    let mut abs = flush(para(
        "Abstract—In about 150–250 words: the problem, the approach, the main result and its significance. Do not cite references or use symbols in the abstract.",
        chr(9.0, true, false),
    ));
    abs.props.align = Some(Align::Justify);
    let mut terms = flush(para("Index Terms—four or five keywords, alphabetical, separated by commas.", chr(9.0, true, true)));
    terms.props.space_after = Some(6.0);
    let paras = vec![
        center(para("Paper Title in Title Case", chr(24.0, false, false))),
        author,
        last_of_title,
        abs,
        terms,
        styled("I. Introduction", "Heading1"),
        plain(
            "IEEE papers number references in order of first citation, in square brackets [1]. Figures and tables are numbered and captioned; table captions go above the table, figure captions below.",
        ),
        styled("II. Method", "Heading1"),
        styled("A. Setup", "Heading2"),
        plain("Describe the system, data and experimental setup."),
        styled("III. Results", "Heading1"),
        plain("Present results with figures and tables; refer to them as Fig. 1 and Table I."),
        styled("IV. Conclusion", "Heading1"),
        plain("Summarize the contribution and future work."),
        styled("Acknowledgment", "Heading1"),
        plain("Funding and thanks (removed for blind review)."),
        styled("References", "Heading1"),
        flush(plain("Insert the bibliography here (Research › Bibliography).")),
    ];
    let mut d = build(
        paras,
        &look,
        &[
            ("Heading1", 10.0, false, false, Align::Center),
            ("Heading2", 10.0, false, true, Align::Left),
            ("Heading3", 10.0, false, true, Align::Left),
        ],
    );
    if let Some(mut h) = d.styles.get("Heading1").cloned() {
        h.chr.small_caps = Some(true);
        d.styles.upsert(h);
    }
    // The title block spans the page (its own section); the paper below runs in two columns.
    let first = Box::new(d.last_section.clone());
    if let Some(Block::Para(p)) =
        d.body.iter_mut().position(|b| b.as_para().is_some_and(|p| p.text.is_empty())).and_then(|i| d.body.get_mut(i)).map(Arc::make_mut)
    {
        p.section = Some(first);
    }
    d.last_section.start = SectionStart::Continuous;
    d.last_section.columns = Columns { count: 2, space: 14.4, separator: false, widths: Vec::new() };
    d
}

fn abnt() -> Document {
    let look =
        Look { font: "Times New Roman", size: 12.0, line: 1.5, margins: [85.0, 57.0, 57.0, 85.0], a4: true, indent_first: 35.4, justify: true };
    let mut resumo =
        flush(para("Resumo: de 100 a 250 palavras, em parágrafo único: objetivo, método, resultados e conclusões.", chr(12.0, false, false)));
    resumo.props.line_spacing = Some(LineSpacing::Multiple(1.0));
    let mut chave = flush(para("Palavras-chave: palavra; palavra; palavra.", chr(12.0, false, false)));
    chave.props.space_after = Some(12.0);
    let paras = vec![
        center(para("TÍTULO DO ARTIGO: SUBTÍTULO", chr(12.0, true, false))),
        center(plain("")),
        {
            let mut a = plain("Nome do Autor¹\nNome do Coautor²");
            a.props.align = Some(Align::Right);
            a.props.indent_first = Some(0.0);
            a
        },
        center(plain("")),
        resumo,
        chave,
        styled("1 INTRODUÇÃO", "Heading1"),
        plain("Apresente o tema, o problema, os objetivos e a justificativa. Citações no texto seguem a ABNT NBR 10520: (SOBRENOME, ano, p. x)."),
        styled("2 REFERENCIAL TEÓRICO", "Heading1"),
        plain("Discuta os autores que fundamentam o trabalho."),
        styled("3 METODOLOGIA", "Heading1"),
        plain("Descreva o tipo de pesquisa, os participantes, os instrumentos e a análise."),
        styled("4 RESULTADOS E DISCUSSÃO", "Heading1"),
        plain("Apresente e discuta os resultados. Tabelas e figuras são numeradas, com título acima e fonte abaixo."),
        styled("5 CONSIDERAÇÕES FINAIS", "Heading1"),
        plain("Retome os objetivos e apresente as conclusões."),
        styled("REFERÊNCIAS", "Heading1"),
        flush(plain(
            "Insira a bibliografia aqui (Research › Bibliography). As referências seguem a ABNT NBR 6023, em ordem alfabética, alinhadas à esquerda.",
        )),
        grey(flush(para("¹ Titulação e instituição do autor. E-mail.", chr(10.0, false, false)))),
    ];
    build(
        paras,
        &look,
        &[("Heading1", 12.0, true, false, Align::Left), ("Heading2", 12.0, false, false, Align::Left), ("Heading3", 12.0, false, true, Align::Left)],
    )
}

fn chicago() -> Document {
    let look = Look { font: "Times New Roman", size: 12.0, line: 2.0, margins: [INCH; 4], a4: false, indent_first: 36.0, justify: false };
    let paras = vec![
        center(plain("")),
        center(plain("")),
        center(para("Title of the Paper", chr(12.0, true, false))),
        center(plain("Subtitle")),
        center(plain("")),
        center(plain("Student Name")),
        center(plain("Course Name")),
        center(plain("Date")),
        page_break_before(plain(
            "Chicago style uses footnotes (References › Insert Footnote) or author-date citations. Notes are numbered consecutively; the bibliography lists every source alphabetically.",
        )),
        styled("Background", "Heading1"),
        plain("Set out the context and the question."),
        styled("Argument", "Heading1"),
        plain("Develop the argument with evidence and notes."),
        styled("Conclusion", "Heading1"),
        plain("Draw it together."),
        page_break_before(center(para("Bibliography", chr(12.0, true, false)))),
        flush(plain("Insert the bibliography here (Research › Bibliography).")),
    ];
    build(
        paras,
        &look,
        &[("Heading1", 12.0, true, false, Align::Left), ("Heading2", 12.0, false, true, Align::Left), ("Heading3", 12.0, false, false, Align::Left)],
    )
}

fn nature() -> Document {
    let look = Look { font: "Arial", size: 11.0, line: 1.5, margins: [INCH; 4], a4: true, indent_first: 0.0, justify: false };
    let mut summary = flush(para(
        "Summary paragraph (about 200 words, referenced): basic background for any reader, more detailed background, the general problem, then 'Here we show…', the main result, and its broader implications.",
        chr(11.0, true, false),
    ));
    summary.props.space_after = Some(12.0);
    let paras = vec![
        flush(para("Title of no more than 75 characters, without jargon", chr(18.0, true, false))),
        flush(plain("First Author¹, Second Author² & Last Author¹")),
        grey(flush(para("¹Department, Institution, City, Country. ²Department, Institution, City, Country.", chr(9.0, false, false)))),
        flush(plain("")),
        summary,
        plain(
            "Main text: about 2,000–2,500 words without headings in a letter, or with a few short headings in an article. References are numbered in order of appearance as superscripts.",
        ),
        styled("Methods", "Heading1"),
        plain("Methods go after the main text, with subheadings in bold. Include data and code availability."),
        styled("References", "Heading1"),
        flush(plain("Insert the bibliography here (Research › Bibliography).")),
        styled("Acknowledgements", "Heading1"),
        plain("Funding and thanks."),
        styled("Author contributions", "Heading1"),
        plain("Who did what, by initials."),
        styled("Competing interests", "Heading1"),
        plain("The authors declare no competing interests."),
        styled("Figure legends", "Heading1"),
        plain("Fig. 1 | A short title sentence. Then a description of what the figure shows, with the statistics used."),
    ];
    build(
        paras,
        &look,
        &[("Heading1", 11.0, true, false, Align::Left), ("Heading2", 11.0, true, false, Align::Left), ("Heading3", 11.0, false, true, Align::Left)],
    )
}

fn elsevier() -> Document {
    let look = Look { font: "Times New Roman", size: 12.0, line: 2.0, margins: [INCH; 4], a4: true, indent_first: 0.0, justify: false };
    let paras = vec![
        flush(para("Title: concise and informative", chr(16.0, true, false))),
        flush(plain("First Author a,*, Second Author b")),
        grey(flush(para(
            "a Department, University, City, Country\nb Institute, City, Country\n* Corresponding author. E-mail: author@example.org",
            chr(10.0, false, false),
        ))),
        styled("Highlights", "Heading1"),
        flush(plain("• Three to five bullet points\n• Each at most 85 characters, spaces included\n• They say what is new")),
        styled("Abstract", "Heading1"),
        flush(plain("No more than 250 words: purpose, main results and conclusions. Avoid references and abbreviations.")),
        flush(plain("Keywords: one; two; three; four")),
        styled("1. Introduction", "Heading1"),
        plain("State the objectives and give an adequate background, avoiding a detailed literature survey."),
        styled("2. Material and methods", "Heading1"),
        plain("Enough detail to reproduce the work; methods already published are referenced."),
        styled("3. Results", "Heading1"),
        plain("Clear and concise."),
        styled("4. Discussion", "Heading1"),
        plain("Explore the significance of the results."),
        styled("5. Conclusions", "Heading1"),
        plain("The main conclusions, in a short section."),
        styled("CRediT authorship contribution statement", "Heading1"),
        plain("First Author: Conceptualization, Writing – original draft. Second Author: Methodology, Writing – review & editing."),
        styled("Declaration of competing interest", "Heading1"),
        plain(
            "The authors declare that they have no known competing financial interests or personal relationships that could have appeared to influence the work reported in this paper.",
        ),
        styled("References", "Heading1"),
        flush(plain("Insert the bibliography here (Research › Bibliography).")),
    ];
    build(
        paras,
        &look,
        &[("Heading1", 12.0, true, false, Align::Left), ("Heading2", 12.0, false, true, Align::Left), ("Heading3", 12.0, false, true, Align::Left)],
    )
}

fn springer() -> Document {
    let look =
        Look { font: "Times New Roman", size: 10.0, line: 1.15, margins: [INCH, 90.0, INCH, 90.0], a4: true, indent_first: 12.0, justify: true };
    let paras = vec![
        center(para("Title of the Article", chr(16.0, true, false))),
        center(plain("First Author¹ · Second Author²")),
        grey(center(para("¹ Institution, City, Country · ² Institution, City, Country", chr(9.0, false, false)))),
        flush(para("Abstract", chr(10.0, true, false))),
        flush(para("150 to 250 words, without undefined abbreviations or references.", chr(9.0, false, false))),
        flush(para("Keywords Four · to · six · keywords", chr(9.0, false, false))),
        styled("1 Introduction", "Heading1"),
        plain("Headings are numbered with decimal numbers, no full stop after the last digit."),
        styled("2 Methods", "Heading1"),
        styled("2.1 Data", "Heading2"),
        plain("Describe the data."),
        styled("3 Results", "Heading1"),
        plain("Present the results."),
        styled("4 Discussion", "Heading1"),
        plain("Discuss them."),
        styled("Declarations", "Heading1"),
        plain("Funding, conflicts of interest, ethics approval, consent, availability of data and code, authors' contributions."),
        styled("References", "Heading1"),
        flush(plain("Insert the bibliography here (Research › Bibliography).")),
    ];
    build(
        paras,
        &look,
        &[("Heading1", 12.0, true, false, Align::Left), ("Heading2", 10.0, true, false, Align::Left), ("Heading3", 10.0, false, true, Align::Left)],
    )
}

fn thesis() -> Document {
    let look =
        Look { font: "Times New Roman", size: 12.0, line: 1.5, margins: [85.0, 57.0, 57.0, 85.0], a4: true, indent_first: 35.4, justify: true };
    let paras = vec![
        center(para("UNIVERSIDADE", chr(12.0, true, false))),
        center(para("PROGRAMA DE PÓS-GRADUAÇÃO", chr(12.0, true, false))),
        center(plain("")),
        center(plain("NOME DO AUTOR")),
        center(plain("")),
        center(plain("")),
        center(para("TÍTULO DA TESE: SUBTÍTULO", chr(12.0, true, false))),
        center(plain("")),
        center(plain("")),
        center(plain("CIDADE\nANO")),
        page_break_before(center(para("RESUMO", chr(12.0, true, false)))),
        flush(plain("Resumo em parágrafo único, de 150 a 500 palavras.")),
        flush(plain("Palavras-chave: palavra; palavra; palavra.")),
        page_break_before(center(para("ABSTRACT", chr(12.0, true, false)))),
        flush(plain("The abstract in English.")),
        flush(plain("Keywords: word; word; word.")),
        page_break_before(center(para("SUMÁRIO", chr(12.0, true, false)))),
        flush(plain("Insira o sumário aqui (References › Table of Contents).")),
        page_break_before(styled("1 INTRODUÇÃO", "Heading1")),
        plain("Cada capítulo começa em nova página."),
        page_break_before(styled("2 REVISÃO DA LITERATURA", "Heading1")),
        plain(""),
        page_break_before(styled("3 METODOLOGIA", "Heading1")),
        plain(""),
        page_break_before(styled("4 RESULTADOS", "Heading1")),
        plain(""),
        page_break_before(styled("5 CONCLUSÃO", "Heading1")),
        plain(""),
        page_break_before(center(para("REFERÊNCIAS", chr(12.0, true, false)))),
        flush(plain("Insira a bibliografia aqui (Research › Bibliography).")),
    ];
    build(
        paras,
        &look,
        &[("Heading1", 12.0, true, false, Align::Left), ("Heading2", 12.0, false, false, Align::Left), ("Heading3", 12.0, false, true, Align::Left)],
    )
}

fn response_letter_template() -> Document {
    let look = Look { font: "Times New Roman", size: 12.0, line: 1.15, margins: [INCH; 4], a4: false, indent_first: 0.0, justify: false };
    let paras = vec![
        flush(para("Response to Reviewers", chr(18.0, true, false))),
        flush(plain("Manuscript: “Title of the manuscript” (ID)")),
        flush(plain("")),
        flush(plain("Dear Editor,")),
        flush(plain(
            "We thank the editor and the reviewers for their careful reading and helpful comments. Below we answer each comment in turn; reviewer comments are in italics, our responses follow, and changes in the manuscript are noted with their location.",
        )),
        styled("Reviewer 1", "Heading1"),
        flush(para("Comment 1.1: The reviewer's comment, quoted in full.", chr(12.0, false, true))),
        flush(plain("Response: How you addressed it.")),
        grey(flush(plain("Changes: Section 3, page 7, lines 120–134."))),
        styled("Reviewer 2", "Heading1"),
        flush(para("Comment 2.1: …", chr(12.0, false, true))),
        flush(plain("Response: …")),
    ];
    build(paras, &look, &[("Heading1", 13.0, true, false, Align::Left)])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_academic_template_builds_with_its_style() {
        for (id, _) in LIST {
            let d = document(id).unwrap_or_else(|| panic!("{id}"));
            assert!(!d.body.is_empty(), "{id}");
            if id != "acad-response" {
                assert!(d.settings.bib_style.is_some(), "{id}");
            }
            let normal = d.styles.get("Normal").unwrap();
            assert!(normal.chr.size.is_some(), "{id}");
        }
        let ieee = document("acad-ieee").unwrap();
        assert_eq!(ieee.last_section.columns.count, 2);
        assert!(ieee.body.iter().any(|b| b.as_para().is_some_and(|p| p.section.is_some())), "the title block is its own section");
    }
}
