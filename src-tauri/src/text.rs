use jieba_rs::Jieba;
use std::sync::OnceLock;

static JIEBA: OnceLock<Jieba> = OnceLock::new();

/// Break opportunities for headings. Keep closing punctuation with the preceding
/// word and opening punctuation with the following word, without changing text.
fn title_tokens(text: &str) -> Vec<String> {
    let jieba = JIEBA.get_or_init(Jieba::new);
    let mut tokens: Vec<String> = Vec::new();
    let mut prefix = String::new();

    for word in jieba.cut(text, true) {
        if word.contains(['\n', '\r']) {
            if !prefix.is_empty() {
                tokens.push(std::mem::take(&mut prefix));
            }
            tokens.push(word.to_owned());
        } else if word.chars().all(|c| "（([【《〈「『“‘".contains(c)) {
            prefix.push_str(word);
        } else if prefix.is_empty()
            && word
                .chars()
                .all(|c| "，。！？；：、,.!?;:）)]】》〉」』”’…—".contains(c))
        {
            if let Some(previous) = tokens.last_mut().filter(|s| !s.contains(['\n', '\r'])) {
                previous.push_str(word);
            } else {
                prefix.push_str(word);
            }
        } else {
            prefix.push_str(word);
            tokens.push(std::mem::take(&mut prefix));
        }
    }
    if !prefix.is_empty() {
        tokens.push(prefix);
    }
    tokens
}

#[tauri::command]
pub async fn segment_title(text: String) -> Result<Vec<String>, String> {
    // Loading the embedded dictionary on first use must not block the UI thread.
    tauri::async_runtime::spawn_blocking(move || title_tokens(&text))
        .await
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::title_tokens;

    #[test]
    fn screenshot_title_keeps_final_word_intact() {
        let title = "关于以智能参谋助手统一认知攻防作业入口的几点建议";
        let tokens = title_tokens(title);
        assert_eq!(tokens.concat(), title);
        assert_eq!(tokens.last().unwrap(), "建议");
    }

    #[test]
    fn punctuation_stays_attached_to_words() {
        let tokens = title_tokens("《知识库》：构建模型，提升效率！");
        assert_eq!(tokens.concat(), "《知识库》：构建模型，提升效率！");
        assert!(tokens
            .iter()
            .any(|s| s.starts_with('《') && s.ends_with('：')));
        assert!(!tokens.iter().any(|s| s == "，" || s == "！" || s == "《"));
    }

    #[test]
    fn preserves_spaces_manual_breaks_and_mixed_text() {
        for title in [
            "",
            "AI 助手 2.0\n合理换行",
            "一、当前需要关注的问题",
            "（模型）\r\n下一行",
            "标题 <script>&内容",
        ] {
            assert_eq!(title_tokens(title).concat(), title);
        }
    }
}
