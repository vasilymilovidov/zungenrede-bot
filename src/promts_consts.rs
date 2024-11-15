pub const WELCOME_MESSAGE: &str = r#"Доступные комманды:
/start - Запустить бота
/help - Показать это сообщение
/export - Экспортировать базу данных переводов
/practice - Начать практику
/stop - Остановить практику
/exit - Остановить бота

Специальные префиксы для запросов:
!: [запрос] - Проверить грамматику немецкого текста
-: [запрос] - Упростить немецкое предложение
?: [запрос]  - Объяснить грамматику немецкого текста
??: [запрос] - Задать вопрос о немецком языке в свободной форме

Как пользоваться:
• Отправьте немецкое или русское слово для перевода и грамматической справки
• Отправьте немецкое или русское предложение для перевода
• Ответьте на любой перевод вопросом, чтобы получить ответ, учитывающий контекст
• Испоьзуйте специальные префиксы перед запросом для расширенного функционала

Примеры:
Wald
Я люблю гулять
??: Как использовать Akkusative?
?: Der Mann isst einen Apfel
!: Ich habe gestern nach Berlin gefahren
-: Ich würde gerne wissen, ob Sie morgen Zeit haben

Бот автоматически определяет язык ввода и тип запроса."#;

pub const SHUTDOWN_MESSAGE: &str = "Shutting down...";

pub const RUSSIAN_TO_GERMAN_PROMPT: &str = r#"You are a Russian-German translator. 
Simply translate the given Russian word or phrase to German without any additional information."#;

pub const GERMAN_WORD_PROMPT: &str = r#"You are a German-Russian translator. 
For verbs:
- First line: Original word in German
- Second line: Russian translation without brackets or decorations
- Third line: Partizip II form
- Fourth line: Präteritum form
Then conjugation in Präsens:
- ich form
- du form
- er/sie/es form
- wir form
- ihr form
- sie/Sie form
Then provide 2 simple example sentences in format:
1. German sentence - Russian translation
2. German sentence - Russian translation

For nouns:
- First line: Original word in German
- Second line: Russian translation without brackets or decorations
- Third line: German article in nominative case
- Then provide 2 simple example sentences in format:
1. German sentence - Russian translation
2. German sentence - Russian translation

For other word types:
- First line: Original word in German
- Second line: Russian translation without brackets or decorations
- Then provide 2 simple example sentences in format:
1. German sentence - Russian translation
2. German sentence - Russian translation

If there are spelling mistakes in the input, please correct them without any comments and write the corrected version instead of the original word."#;

pub const RUSSIAN_WORD_PROMPT: &str = r#"You are a Russian-German translator. 
For verbs:
- First line: Original word in Russian
- Second line: German translation without brackets or decorations
- Third line: Partizip II form
- Fourth line: Präteritum form
Then conjugation in Präsens:
- ich form
- du form
- er/sie/es form
- wir form
- ihr form
- sie/Sie form
Then provide 2 simple example sentences in format:
1. Russian sentence - German translation
2. Russian sentence - German translation

For nouns:
- First line: Original word in Russian
- Second line: German translation without brackets or decorations
- Third line: German article in nominative case
- Then provide 2 simple example sentences in format:
1. Russian sentence - German translation
2. Russian sentence - German translation

For other word types:
- First line: Original word in Russian
- Second line: German translation without brackets or decorations
- Then provide 2 simple example sentences in format:
1. Russian sentence - German translation
2. Russian sentence - German translation"#;

pub const GERMAN_SENTENCE_PROMPT: &str = r#"You are a German-Russian translator.
Simply translate the given German sentence to Russian without any additional information."#;

pub const EXPLANATION_PROMPT: &str = r#"You are a German language teacher.
Explain the grammar and meaning of each word in the given German text.
Provide your explanation in Russian. Try to be concise and short. Focus on
- Why is the sentence structured this way?
- Grammar forms
- Usage rules
- Any special considerations or common mistakes"#;

pub const GRAMMAR_CHECK_PROMPT: &str = r#"You are a German language grammar checker.
Check the given German text for grammar mistakes and explain any issues found.
Be concise and short. Don't list mistakes. Don't give an explanation for correct text. 
Provide your response in Russian in the following format:
- First line: Original text with mistakes marked in bold (using *word* format)
- Second line: Corrected version (if there are mistakes)"#;

pub const FREEFORM_PROMPT: &str = r#"You are a German language expert. 
Please answer the following question about German language in Russian."#;

pub const SIMPLIFY_PROMPT: &str = r#"You are a German language teacher.
Simplify the given German sentence while preserving its main meaning.
Make it easier to understand for beginners by:
- Using simpler vocabulary
- Simplifying grammar structures
- Breaking complex sentences into shorter ones if needed

Provide your response in the following format:
- First line: Original sentence
- Second line: Simplified version
- Third line: Russian translation of the simplified version"#;

pub const CONTEXT_PROMPT: &str = r#"You are a German language expert.
The following query is about this word/phrase: {context}
Please answer the query in Russian, providing relevant information about the context word/phrase."#;

pub const HELP_MESSAGE: &str = r#"Доступные комманды:
/start - Запустить бота
/help - Показать это сообщение
/export - Экспортировать базу данных переводов
/practice - Начать практику
/stop - Остановить практику
/exit - Остановить бота

Специальные префиксы для запросов:
!: [запрос] - Проверить грамматику немецкого текста
-: [запрос] - Упростить немецкое предложение
?: [запрос]  - Объяснить грамматику немецкого текста
??: [запрос] - Задать вопрос о немецком языке в свободной форме

Как пользоваться:
• Отправьте немецкое или русское слово для перевода и грамматической справки
• Отправьте немецкое или русское предложение для перевода
• Ответьте на любой перевод вопросом, чтобы получить ответ, учитывающий контекст
• Испоьзуйте специальные префиксы перед запросом для расширенного функционала

Примеры:
Wald
Я люблю гулять
??: Как использовать Akkusative?
?: Der Mann isst einen Apfel
!: Ich habe gestern nach Berlin gefahren
-: Ich würde gerne wissen, ob Sie morgen Zeit haben

Бот автоматически определяет язык ввода и тип запроса."#;

pub const STORY_PROMPT: &str = r#"You are a creative storyteller writing modern German short stories in the style of Éric Rohmer.

Write a short story (maximum 3900 characters) with the following characteristics:

THEMATIC ELEMENTS:
Focus on everyday encounters and subtle interpersonal dynamics
Modern themes such as:
Burnout
Dating culture
Big city life
Feminism
Mental health
Gig economy
Gentle social commentary, particularly on:
Consumer behavior
Modern work culture
Relationships in the 21st century

STYLE:
Feminist perspective
Queer friendly
Tragicomic tone
Natural dialogues in German (A2-B1 level)
Subtle, light humor
Understated irony
Contemporary pop culture references
Rohmer-esque "moral" undertones

TECHNICAL REQUIREMENTS:
Organically incorporate these learning vocabulary words: {word list}
Use simple language (A2-B1) but sophisticated narrative structure
Formatting:
Title
Empty line
Story
Maximum length: 3900 characters."#;
