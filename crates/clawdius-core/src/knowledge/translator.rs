//! Simple translator with caching

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::concepts::Language;
use crate::Result;

static TRANSLATION_DICT: &[(&str, &str, &str)] = &[
    ("machine learning", "zh", "机器学习"),
    ("machine learning", "de", "Maschinelles Lernen"),
    ("machine learning", "fr", "Apprentissage automatique"),
    ("machine learning", "es", "Aprendizaje automático"),
    ("machine learning", "jp", "機械学習"),
    ("machine learning", "ru", "Машинное обучение"),
    ("artificial intelligence", "zh", "人工智能"),
    ("artificial intelligence", "de", "Künstliche Intelligenz"),
    ("artificial intelligence", "fr", "Intelligence artificielle"),
    ("artificial intelligence", "es", "Inteligencia artificial"),
    ("artificial intelligence", "jp", "人工知能"),
    ("artificial intelligence", "ru", "Искусственный интеллект"),
    ("deep learning", "zh", "深度学习"),
    ("deep learning", "de", "Tiefes Lernen"),
    ("deep learning", "fr", "Apprentissage profond"),
    ("deep learning", "jp", "ディープラーニング"),
    ("neural network", "zh", "神经网络"),
    ("neural network", "de", "Neuronales Netz"),
    ("neural network", "fr", "Réseau neuronal"),
    ("neural network", "jp", "ニューラルネットワーク"),
    ("algorithm", "zh", "算法"),
    ("algorithm", "de", "Algorithmus"),
    ("algorithm", "fr", "Algorithme"),
    ("data", "zh", "数据"),
    ("data", "de", "Daten"),
    ("data", "fr", "Données"),
    ("computer", "zh", "电脑"),
    ("computer", "de", "Computer"),
    ("computer", "fr", "Ordinateur"),
    ("software", "zh", "软件"),
    ("software", "de", "Software"),
    ("software", "fr", "Logiciel"),
    ("programming", "zh", "编程"),
    ("programming", "de", "Programmierung"),
    ("programming", "fr", "Programmation"),
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Translator {
    cache: HashMap<(String, Language, Language), String>,
}

impl Default for Translator {
    fn default() -> Self {
        Self::new()
    }
}

impl Translator {
    #[must_use]
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
        }
    }

    pub async fn translate(&mut self, text: &str, from: Language, to: Language) -> Result<String> {
        if from == to {
            return Ok(text.to_string());
        }

        let cache_key = (text.to_lowercase(), from, to);
        if let Some(cached) = self.cache.get(&cache_key) {
            return Ok(cached.clone());
        }

        let result = self.lookup_translation(text, from, to);

        self.cache.insert(cache_key, result.clone());

        Ok(result)
    }

    fn lookup_translation(&self, text: &str, from: Language, to: Language) -> String {
        let text_lower = text.to_lowercase();

        if from == Language::EN {
            let target_code = to.to_string();
            for (term, lang, translation) in TRANSLATION_DICT {
                if text_lower.contains(term) && *lang == target_code {
                    return text_lower.replace(term, translation);
                }
            }
        } else if to == Language::EN {
            let source_code = from.to_string();
            for (term, lang, translation) in TRANSLATION_DICT {
                if *lang == source_code && text_lower.contains(translation) {
                    return text_lower.replace(translation, term);
                }
            }
        } else {
            let source_code = from.to_string();
            let target_code = to.to_string();

            for (term, src_lang, src_trans) in TRANSLATION_DICT {
                if *src_lang == source_code && text_lower.contains(src_trans) {
                    for (en_term, tgt_lang, tgt_trans) in TRANSLATION_DICT {
                        if *term == *en_term && *tgt_lang == target_code {
                            return text_lower.replace(src_trans, tgt_trans);
                        }
                    }
                    let english = text_lower.replace(src_trans, term);
                    for (en_term, tgt_lang, tgt_trans) in TRANSLATION_DICT {
                        if *en_term == *term && *tgt_lang == target_code {
                            return english.replace(term, tgt_trans);
                        }
                    }
                    return english;
                }
            }
        }

        format!("[{text}]")
    }

    pub async fn translate_batch(
        &mut self,
        texts: &[&str],
        from: Language,
        to: Language,
    ) -> Result<Vec<String>> {
        let mut results = Vec::with_capacity(texts.len());
        for text in texts {
            results.push(self.translate(text, from, to).await?);
        }
        Ok(results)
    }

    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }

    #[must_use]
    pub fn cache_size(&self) -> usize {
        self.cache.len()
    }
}

#[must_use]
pub fn detect_language(text: &str) -> Option<Language> {
    let text = text.trim();

    if text.is_empty() {
        return None;
    }

    let has_cjk = text.chars().any(|c| {
        ('\u{4E00}'..='\u{9FFF}').contains(&c)
            || ('\u{3400}'..='\u{4DBF}').contains(&c)
            || ('\u{20000}'..='\u{2A6DF}').contains(&c)
    });

    let has_hiragana = text.chars().any(|c| ('\u{3040}'..='\u{309F}').contains(&c));
    let has_katakana = text.chars().any(|c| ('\u{30A0}'..='\u{30FF}').contains(&c));
    let has_hangul = text.chars().any(|c| ('\u{AC00}'..='\u{D7AF}').contains(&c));
    let has_cyrillic = text.chars().any(|c| ('\u{0400}'..='\u{04FF}').contains(&c));
    let has_arabic = text.chars().any(|c| ('\u{0600}'..='\u{06FF}').contains(&c));
    let has_hebrew = text.chars().any(|c| ('\u{0590}'..='\u{05FF}').contains(&c));

    if has_hiragana || has_katakana {
        return Some(Language::JP);
    }

    if has_hangul {
        return Some(Language::KO);
    }

    if has_cjk && text.chars().any(|c| ('\u{4E00}'..='\u{9FFF}').contains(&c)) {
        return Some(Language::ZH);
    }

    if has_cyrillic {
        let russian_chars = text
            .chars()
            .filter(|&c| ('\u{0400}'..='\u{04FF}').contains(&c))
            .count();
        let other_cyrillic = text
            .chars()
            .filter(|&c| {
                ('\u{0400}'..='\u{04FF}').contains(&c) && !('\u{0410}'..='\u{044F}').contains(&c)
            })
            .count();

        if russian_chars > 0 && other_cyrillic == 0 {
            return Some(Language::RU);
        }
    }

    if has_arabic {
        return Some(Language::AR);
    }

    if has_hebrew {
        return Some(Language::FA);
    }

    let lower = text.to_lowercase();
    let common_en = [
        "the", "is", "are", "and", "or", "in", "on", "at", "to", "for",
    ];
    let common_de = [
        "der", "die", "das", "ist", "sind", "und", "oder", "ein", "eine",
    ];
    let common_fr = ["le", "la", "les", "est", "sont", "et", "ou", "un", "une"];
    let common_es = ["el", "la", "los", "es", "son", "y", "o", "un", "una"];
    let common_it = ["il", "la", "lo", "è", "sono", "e", "o", "un", "una"];
    let common_pt = ["o", "a", "os", "é", "são", "e", "ou", "um", "uma"];
    let common_nl = ["de", "het", "is", "zijn", "en", "of", "een"];
    let common_pl = ["jest", "są", "i", "lub", "nie", "to", "na"];
    let common_cs = ["je", "jsou", "a", "nebo", "na", "to"];
    let common_tr = ["bir", "ve", "veya", "bu", "şu", "için"];

    let words: Vec<&str> = lower.split_whitespace().collect();

    let count_common = |common: &[&str]| words.iter().filter(|&&w| common.contains(&w)).count();

    let scores = [
        (Language::EN, count_common(&common_en)),
        (Language::DE, count_common(&common_de)),
        (Language::FR, count_common(&common_fr)),
        (Language::ES, count_common(&common_es)),
        (Language::IT, count_common(&common_it)),
        (Language::PT, count_common(&common_pt)),
        (Language::NL, count_common(&common_nl)),
        (Language::PL, count_common(&common_pl)),
        (Language::CS, count_common(&common_cs)),
        (Language::TR, count_common(&common_tr)),
    ];

    scores
        .into_iter()
        .filter(|(_, count)| *count > 0)
        .max_by_key(|(_, count)| *count)
        .map(|(lang, _)| lang)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_translate_same_language() {
        let mut translator = Translator::new();
        let result = translator
            .translate("Hello", Language::EN, Language::EN)
            .await
            .unwrap();
        assert_eq!(result, "Hello");
    }

    #[tokio::test]
    async fn test_translate_with_dictionary() {
        let mut translator = Translator::new();
        let result = translator
            .translate("machine learning", Language::EN, Language::ZH)
            .await
            .unwrap();
        assert_eq!(result, "机器学习");
    }

    #[tokio::test]
    async fn test_translate_caching() {
        let mut translator = Translator::new();

        translator
            .translate("test", Language::EN, Language::ZH)
            .await
            .unwrap();
        assert_eq!(translator.cache_size(), 1);

        translator
            .translate("test", Language::EN, Language::ZH)
            .await
            .unwrap();
        assert_eq!(translator.cache_size(), 1);
    }

    #[test]
    fn test_detect_language_english() {
        let result = detect_language("The quick brown fox jumps over the lazy dog");
        assert_eq!(result, Some(Language::EN));
    }

    #[test]
    fn test_detect_language_chinese() {
        assert_eq!(detect_language("这是一个测试"), Some(Language::ZH));
        assert_eq!(detect_language("机器学习很重要"), Some(Language::ZH));
    }

    #[test]
    fn test_detect_language_japanese() {
        assert_eq!(detect_language("これはテストです"), Some(Language::JP));
        assert_eq!(detect_language("機械学習を学ぶ"), Some(Language::JP));
    }

    #[test]
    fn test_detect_language_korean() {
        assert_eq!(detect_language("이것은 테스트입니다"), Some(Language::KO));
    }

    #[test]
    fn test_detect_language_russian() {
        assert_eq!(detect_language("Это тест"), Some(Language::RU));
        assert_eq!(detect_language("Машинное обучение"), Some(Language::RU));
    }

    #[test]
    fn test_detect_language_arabic() {
        assert_eq!(detect_language("هذا اختبار"), Some(Language::AR));
    }

    #[test]
    fn test_detect_language_german() {
        assert_eq!(detect_language("Das ist ein Test"), Some(Language::DE));
    }

    #[test]
    fn test_detect_language_french() {
        assert_eq!(detect_language("Ceci est un test"), Some(Language::FR));
    }
}
