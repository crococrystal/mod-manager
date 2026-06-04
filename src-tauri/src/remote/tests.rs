use super::*;

fn project(title: &str, slug: &str, project_type: &str) -> ProviderProject {
    ProviderProject {
        id: slug.to_string(),
        slug: Some(slug.to_string()),
        title: Some(title.to_string()),
        project_type: Some(project_type.to_string()),
    }
}

#[test]
fn modrinth_match_accepts_exact_mod() {
    let project = project("FTB Quests", "ftb-quests", "mod");
    assert!(modrinth_project_matches(&project, "FTB Quests"));
}

#[test]
fn modrinth_match_rejects_resource_packs() {
    let project = project("FTB Quests 中文翻译", "ftb-quests-zh_cn", "resourcepack");
    assert!(!modrinth_project_matches(&project, "FTB Quests"));
}

#[test]
fn modrinth_match_rejects_similar_addons() {
    let project = project("FTB Quests Freeze Fix", "ftb-quests-freeze-fix", "mod");
    assert!(!modrinth_project_matches(&project, "FTB Quests"));
}

#[test]
fn strip_version_suffixes_removes_semver_tail() {
    assert_eq!(
        strip_version_suffixes("FramedBlocks-10.3.2"),
        "FramedBlocks"
    );
}

#[test]
fn provider_match_accepts_jar_name_with_version() {
    let project = project("FramedBlocks", "framedblocks", "mod");
    assert!(provider_project_matches(&project, "FramedBlocks-10.3.2"));
}

#[test]
fn search_queries_prefer_name_without_version() {
    let queries = search_queries_from_display_name("FramedBlocks-10.3.2");
    assert_eq!(queries.first().map(String::as_str), Some("FramedBlocks"));
}

#[test]
fn strip_filename_decorations_removes_emoji_prefix() {
    assert_eq!(
        strip_filename_decorations("💡 -simplylight-1.5.3+1.21.1"),
        "simplylight-1.5.3+1.21.1"
    );
}

#[test]
fn provider_match_accepts_emoji_jar_name() {
    let project = project("Simply Light", "simplylight", "mod");
    assert!(provider_project_matches(
        &project,
        "💡 -simplylight-1.5.3+1.21.1"
    ));
}

#[test]
fn search_queries_strip_emoji_prefix() {
    let queries = search_queries_from_display_name("🖥️ -AdvancedPeripherals-1.21.1-0.7.51b");
    assert_eq!(
        queries.first().map(String::as_str),
        Some("AdvancedPeripherals")
    );
    assert!(queries.iter().any(|query| query == "Advanced Peripherals"));
}

#[test]
fn provider_match_accepts_curseforge_slug_with_dashes() {
    let project = ProviderProject {
        id: "431733".to_string(),
        slug: Some("advanced-peripherals".to_string()),
        title: Some("Advanced Peripherals".to_string()),
        project_type: None,
    };
    assert!(provider_project_matches(
        &project,
        "AdvancedPeripherals-1.21.1-0.7.51b"
    ));
}

#[test]
fn provider_match_crafting_station_jei_jar() {
    let project = project(
        "Crafting Station: JEI Edition",
        "crafting-station-jei-edition",
        "mod",
    );
    assert!(modrinth_project_matches(
        &project,
        "-craftingstation-jei-neoforge-1.21.1-1.6.0"
    ));
}

#[test]
fn search_queries_include_hyphen_spaced_words() {
    let queries = search_queries_from_display_name("-craftingstation-jei-neoforge-1.21.1-1.6.0");
    assert!(queries.iter().any(|q| q == "craftingstation jei"));
}

#[test]
fn search_queries_prefer_spaced_ftb_teams() {
    let queries = search_queries_from_display_name("🌐 📁 -ftb-teams-neoforge-2101.1.2");
    assert_eq!(queries.first().map(String::as_str), Some("ftb teams"));
}

#[test]
fn mod_name_tokens_strip_hotfix_tail() {
    let tokens = mod_name_tokens("🌐🔮-alshanex_familiars-1.21.1_v2.0_HotFix2");
    assert_eq!(tokens, vec!["alshanex", "familiars"]);
}

#[test]
fn search_queries_include_alshanex_underscore() {
    let queries = search_queries_from_display_name("🌐🔮-alshanex_familiars-1.21.1_v2.0_HotFix2");
    assert!(queries.iter().any(|q| q == "alshanex familiars"));
    assert!(queries.iter().any(|q| q == "alshanex_familiars"));
}

#[test]
fn candidate_rank_prefers_ftb_teams() {
    let mut list = vec![
        ProviderCandidate {
            id: "1".to_string(),
            slug: Some("shadows-of-evil".to_string()),
            title: "Shadows Of Evil".to_string(),
            summary: None,
            icon_url: None,
            exact_file_match: false,
            match_score: 0,
        },
        ProviderCandidate {
            id: "2".to_string(),
            slug: Some("ftb-teams".to_string()),
            title: "FTB Teams".to_string(),
            summary: None,
            icon_url: None,
            exact_file_match: false,
            match_score: 0,
        },
    ];
    sort_candidates_by_relevance("-ftb-teams-neoforge-2101.1.2", &mut list);
    assert_eq!(list[0].title, "FTB Teams");
}
