pub const DEAL_EXTRACTION_SYSTEM_PROMPT: &str = "You extract private equity diligence questions from deal documents. Return only strict JSON with no Markdown.";

pub struct DealExtractionPromptVariables<'a> {
    pub deal_id: &'a str,
    pub deal_name: &'a str,
    pub transaction_type: &'a str,
    pub target_company: &'a str,
    pub primary_buyer: &'a str,
    pub deal_sponsor: &'a str,
    pub attached_file_manifest: &'a str,
}

pub fn build_deal_extraction_prompt(variables: DealExtractionPromptVariables<'_>) -> String {
    format!(
        "Deal metadata:\nDeal ID: {}\nDeal name: {}\nTransaction type: {}\nTarget company: {}\nPrimary buyer: {}\nDeal sponsor: {}\n\n\
Review the attached files. Extract only questions explicitly listed beneath a section heading labeled Key Questions or Key Diligence Questions in an attached Word document. Do not create, infer, rewrite, synthesize, or add questions.\n\n\
Return strict JSON with exactly one key: \"keyQuestions\". If there is no qualifying section, return an empty array. Do not include Markdown, commentary, citations, or extra keys.\n\nAttached file manifest:\n{}",
        variables.deal_id,
        variables.deal_name,
        variables.transaction_type,
        variables.target_company,
        variables.primary_buyer,
        variables.deal_sponsor,
        variables.attached_file_manifest,
    )
}
