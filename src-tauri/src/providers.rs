use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::catalog;
use crate::covers::{
    apply_existing_cover, cache_cover_url_with_retry, refetch_mod_cover_after_source_switch,
};
use crate::events::{emit_cover_ready, emit_mod_source_ready};
use crate::file_identity::read_file_identity;
use crate::mods::{source_url, ModEntry};
use crate::remote::{
    curseforge_candidate_for_project, curseforge_fingerprint_match, curseforge_mod_info,
    http_client, list_curseforge_candidates, list_modrinth_candidates,
    modrinth_candidate_for_project, modrinth_version_by_sha512, search_http_client,
    ProviderCandidate,
};
use crate::settings::{read_settings, resolve_paths, InstancePaths, Settings};
use crate::tags::{read_tags, write_tags};
use crate::util::{file_mtime_millis, now_iso, path_string};

mod versions;
pub(crate) use versions::{
    install_version, list_versions, InstallProviderVersionRequest, InstallProviderVersionResult,
    ListProviderVersionsRequest, ProviderVersionsPayload,
};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SearchProviderRequest {
    pub source: String,
    pub display_name: String,
    pub filename: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SwitchModSourceRequest {
    pub key: String,
    pub source: String,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub filename: String,
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub slug: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub icon_url: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SwitchModSourceResult {
    pub key: String,
    pub source: String,
    pub display_name: Option<String>,
    pub source_url: Option<String>,
    pub modrinth_id: Option<String>,
    pub modrinth_version_id: Option<String>,
    pub curseforge_id: Option<String>,
    pub curseforge_file_id: Option<String>,
    pub cover_url: Option<String>,
}

#[derive(Clone, Debug)]
struct SwitchModSourceFollowup {
    settings: Settings,
    paths: InstancePaths,
    catalog_root: Option<PathBuf>,
    key: String,
    source: String,
    display_name: String,
    filename: String,
    project_id: String,
    slug: Option<String>,
    title: Option<String>,
    icon_url: Option<String>,
}

pub(crate) async fn search_candidates(
    app: AppHandle,
    request: SearchProviderRequest,
) -> Result<Vec<ProviderCandidate>, String> {
    let app_handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || -> Result<Vec<ProviderCandidate>, String> {
        let target = request.source.trim().to_ascii_lowercase();
        if target != "modrinth" && target != "curseforge" {
            return Err("Можно искать только на Modrinth или CurseForge.".to_string());
        }

        let display_name = request.display_name.trim();
        if display_name.is_empty() {
            return Err("Не задано имя мода для поиска.".to_string());
        }

        let settings = read_settings(&app_handle)?;
        let client =
            search_http_client().ok_or_else(|| "Не удалось создать HTTP-клиент.".to_string())?;
        let candidates = match target.as_str() {
            "modrinth" => list_modrinth_candidates(&client, display_name),
            "curseforge" => {
                if settings.curseforge_api_key.trim().is_empty() {
                    return Err("Для поиска на CurseForge нужен API key.".to_string());
                }
                list_curseforge_candidates(&client, &settings.curseforge_api_key, display_name)
            }
            _ => unreachable!(),
        };

        if candidates.is_empty() {
            return Err(format!("Ничего не найдено на {}.", target));
        }

        Ok(candidates)
    })
    .await
    .map_err(|error| format!("Поиск поставщика прерван: {error}"))?
}

pub(crate) async fn lookup_fingerprint(
    app: AppHandle,
    request: SearchProviderRequest,
) -> Result<Option<ProviderCandidate>, String> {
    let app_handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || -> Result<Option<ProviderCandidate>, String> {
        let target = request.source.trim().to_ascii_lowercase();
        if target != "modrinth" && target != "curseforge" {
            return Err("Можно искать только на Modrinth или CurseForge.".to_string());
        }
        let filename = request.filename.trim();
        if filename.is_empty() {
            return Ok(None);
        }

        let settings = read_settings(&app_handle)?;
        let paths = resolve_paths(&settings)?;
        Ok(lookup_fingerprint_blocking(
            &settings,
            &paths,
            target.as_str(),
            filename,
        ))
    })
    .await
    .map_err(|error| format!("Проверка файла прервана: {error}"))?
}

pub(crate) async fn switch_source(
    app: AppHandle,
    request: SwitchModSourceRequest,
) -> Result<SwitchModSourceResult, String> {
    let app_handle = app.clone();
    let (result, followup) =
        tauri::async_runtime::spawn_blocking(move || switch_source_quick(&app_handle, request))
            .await
            .map_err(|error| format!("Переключение поставщика прервано: {error}"))??;

    if let Some(followup) = followup {
        run_switch_followup(app, followup);
    }

    Ok(result)
}

fn lookup_fingerprint_blocking(
    settings: &Settings,
    paths: &crate::settings::InstancePaths,
    source: &str,
    filename: &str,
) -> Option<ProviderCandidate> {
    let client = search_http_client()?;
    let identity = read_file_identity(&paths.mods_dir.join(filename)).ok()?;
    match source {
        "curseforge" => {
            if settings.curseforge_api_key.trim().is_empty() {
                return None;
            }
            let found = curseforge_fingerprint_match(
                &client,
                &settings.curseforge_api_key,
                identity.curseforge_fingerprint,
            )?;
            let mut candidate = curseforge_candidate_for_project(
                &client,
                &settings.curseforge_api_key,
                &found.project_id,
            )?;
            candidate.exact_file_match = true;
            candidate.match_score = 1000;
            Some(candidate)
        }
        "modrinth" => {
            let found = modrinth_version_by_sha512(&client, &identity.sha512)?;
            let mut candidate = modrinth_candidate_for_project(&client, &found.project_id)?;
            candidate.exact_file_match = true;
            candidate.match_score = 1000;
            Some(candidate)
        }
        _ => None,
    }
}

fn clean_string(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn switch_source_quick(
    app: &AppHandle,
    request: SwitchModSourceRequest,
) -> Result<(SwitchModSourceResult, Option<SwitchModSourceFollowup>), String> {
    let target = request.source.trim().to_ascii_lowercase();
    if target != "modrinth" && target != "curseforge" {
        return Err("Можно выбрать только Modrinth или CurseForge.".to_string());
    }

    let settings = read_settings(app)?;
    let paths = resolve_paths(&settings)?;
    let filename = request.filename.trim().to_string();
    if !filename.is_empty() && !paths.mods_dir.join(&filename).is_file() {
        return Err("Файл мода не найден в текущей сборке.".to_string());
    }

    let catalog_root = catalog::catalog_root(app).ok();
    let display_name = request.display_name.trim().to_string();
    let chosen_id = request
        .project_id
        .as_ref()
        .and_then(|value| clean_string(value));
    let chosen_slug = request.slug.as_ref().and_then(|value| clean_string(value));
    let chosen_title = request.title.as_ref().and_then(|value| clean_string(value));
    let icon_url = request
        .icon_url
        .as_ref()
        .and_then(|value| clean_string(value));
    let mut tags = read_tags(&paths.tags_path)?;
    let project_id;
    let result;

    {
        let tag = tags.mods.entry(request.key.clone()).or_default();
        let chosen_id = chosen_id
            .or_else(|| match target.as_str() {
                "modrinth" => clean_string(&tag.modrinth_id),
                "curseforge" => clean_string(&tag.curseforge_id),
                _ => None,
            })
            .ok_or_else(|| "Выбери проект из результатов поиска.".to_string())?;
        project_id = chosen_id.clone();

        match target.as_str() {
            "modrinth" => {
                if tag.modrinth_id != chosen_id {
                    tag.modrinth_version_id.clear();
                }
                tag.modrinth_id = chosen_id;
            }
            "curseforge" => {
                if settings.curseforge_api_key.trim().is_empty() {
                    return Err("Для CurseForge нужен API key.".to_string());
                }
                if tag.curseforge_id != chosen_id {
                    tag.curseforge_file_id.clear();
                    if chosen_slug.is_none() {
                        tag.curseforge_slug.clear();
                    }
                }
                tag.curseforge_id = chosen_id;
                if let Some(slug) = chosen_slug.as_ref() {
                    tag.curseforge_slug = slug.clone();
                }
            }
            _ => unreachable!(),
        }
        if let Some(title) = chosen_title.as_ref() {
            tag.provider_title = title.clone();
        }

        tag.source = target.clone();
        tag.updated_at = now_iso();

        let display_name =
            clean_string(&tag.display_name).or_else(|| clean_string(&tag.provider_title));
        let modrinth_id = clean_string(&tag.modrinth_id);
        let modrinth_version_id = clean_string(&tag.modrinth_version_id);
        let curseforge_id = clean_string(&tag.curseforge_id);
        let curseforge_file_id = clean_string(&tag.curseforge_file_id);
        let curseforge_slug = clean_string(&tag.curseforge_slug);
        result = SwitchModSourceResult {
            key: request.key.clone(),
            source: target.clone(),
            display_name,
            source_url: source_url(&target, modrinth_id.as_deref(), curseforge_slug.as_deref()),
            modrinth_id,
            modrinth_version_id,
            curseforge_id,
            curseforge_file_id,
            cover_url: icon_url.clone(),
        };
    }

    tags.updated_at = now_iso();
    write_tags(&paths.tags_path, &tags)?;

    let followup = Some(SwitchModSourceFollowup {
        settings,
        paths,
        catalog_root,
        key: request.key,
        source: target,
        display_name,
        filename,
        project_id,
        slug: chosen_slug,
        title: chosen_title,
        icon_url,
    });

    Ok((result, followup))
}

fn run_switch_followup(app: AppHandle, followup: SwitchModSourceFollowup) {
    tauri::async_runtime::spawn_blocking(move || {
        let Some(client) = http_client() else {
            return;
        };

        refetch_switch_cover(&app, &followup, &client);
        complete_switch_identity(&app, &followup, &client);
    });
}

fn complete_switch_identity(
    app: &AppHandle,
    followup: &SwitchModSourceFollowup,
    client: &reqwest::blocking::Client,
) {
    let mut modrinth_version_id = None;
    let mut curseforge_file_id = None;
    let mut curseforge_slug = followup.slug.clone();
    let mut provider_title = followup.title.clone();

    if !followup.filename.is_empty() {
        if let Ok(identity) = read_file_identity(&followup.paths.mods_dir.join(&followup.filename))
        {
            match followup.source.as_str() {
                "modrinth" => {
                    if let Some(found) = modrinth_version_by_sha512(client, &identity.sha512) {
                        if found.project_id == followup.project_id {
                            modrinth_version_id = Some(found.version_id);
                        }
                    }
                }
                "curseforge" => {
                    if !followup.settings.curseforge_api_key.trim().is_empty() {
                        if let Some(found) = curseforge_fingerprint_match(
                            client,
                            &followup.settings.curseforge_api_key,
                            identity.curseforge_fingerprint,
                        ) {
                            if found.project_id == followup.project_id {
                                curseforge_file_id = Some(found.file_id);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    match followup.source.as_str() {
        "modrinth" if provider_title.is_none() => {
            provider_title = modrinth_candidate_for_project(client, &followup.project_id)
                .map(|candidate| candidate.title);
        }
        "curseforge" if curseforge_slug.is_none() || provider_title.is_none() => {
            if let Some(project) = curseforge_mod_info(
                client,
                &followup.settings.curseforge_api_key,
                &followup.project_id,
            ) {
                if curseforge_slug.is_none() {
                    curseforge_slug = project.slug;
                }
                if provider_title.is_none() {
                    provider_title = project.title;
                }
            }
        }
        _ => {}
    }

    let Ok(mut tags) = read_tags(&followup.paths.tags_path) else {
        return;
    };
    let Some(tag) = tags.mods.get_mut(&followup.key) else {
        return;
    };
    if tag.source != followup.source {
        return;
    }

    let mut changed = false;
    match followup.source.as_str() {
        "modrinth" => {
            if tag.modrinth_id != followup.project_id {
                return;
            }
            if let Some(version_id) = modrinth_version_id {
                if tag.modrinth_version_id != version_id {
                    tag.modrinth_version_id = version_id;
                    changed = true;
                }
            }
            if let Some(title) = provider_title.as_ref() {
                if tag.provider_title != *title {
                    tag.provider_title = title.clone();
                    changed = true;
                }
            }
        }
        "curseforge" => {
            if tag.curseforge_id != followup.project_id {
                return;
            }
            if let Some(file_id) = curseforge_file_id {
                if tag.curseforge_file_id != file_id {
                    tag.curseforge_file_id = file_id;
                    changed = true;
                }
            }
            if let Some(slug) = curseforge_slug {
                if tag.curseforge_slug != slug {
                    tag.curseforge_slug = slug;
                    changed = true;
                }
            }
            if let Some(title) = provider_title.as_ref() {
                if tag.provider_title != *title {
                    tag.provider_title = title.clone();
                    changed = true;
                }
            }
        }
        _ => return,
    }

    if !changed {
        return;
    }

    tag.updated_at = now_iso();
    tags.updated_at = now_iso();
    if write_tags(&followup.paths.tags_path, &tags).is_err() {
        return;
    }

    let tag = tags.mods.get(&followup.key).cloned().unwrap_or_default();
    let display_name =
        clean_string(&tag.display_name).or_else(|| clean_string(&tag.provider_title));
    let modrinth_id = clean_string(&tag.modrinth_id);
    let modrinth_version_id = clean_string(&tag.modrinth_version_id);
    let curseforge_id = clean_string(&tag.curseforge_id);
    let curseforge_file_id = clean_string(&tag.curseforge_file_id);
    let curseforge_slug = clean_string(&tag.curseforge_slug);
    emit_mod_source_ready(
        app,
        &followup.key,
        &followup.source,
        display_name,
        source_url(
            &followup.source,
            modrinth_id.as_deref(),
            curseforge_slug.as_deref(),
        ),
        modrinth_id,
        modrinth_version_id,
        curseforge_id,
        curseforge_file_id,
    );
}

fn refetch_switch_cover(
    app: &AppHandle,
    followup: &SwitchModSourceFollowup,
    client: &reqwest::blocking::Client,
) {
    let (modrinth_id, curseforge_id) = match followup.source.as_str() {
        "modrinth" => (Some(followup.project_id.clone()), None),
        "curseforge" => (None, Some(followup.project_id.clone())),
        _ => return,
    };
    let mut item = cover_item_for_switch(followup, modrinth_id.clone(), curseforge_id.clone());
    apply_existing_cover(&mut item, &followup.paths, followup.catalog_root.as_deref());
    if item.cover_manual {
        return;
    }

    if let Some(url) = followup.icon_url.as_deref() {
        if let Some(path) = cache_cover_url_with_retry(
            client,
            &followup.paths,
            followup.catalog_root.as_deref(),
            &followup.key,
            modrinth_id.as_deref(),
            curseforge_id.as_deref(),
            url,
            true,
        ) {
            let mtime = file_mtime_millis(&path);
            let stored = path_string(path);
            emit_cover_ready(app, &followup.key, &stored, mtime);
            return;
        }
    }

    refetch_mod_cover_after_source_switch(
        app,
        &item,
        &followup.paths,
        followup.catalog_root.as_deref(),
        &followup.settings,
        client,
    );
}

fn cover_item_for_switch(
    followup: &SwitchModSourceFollowup,
    modrinth_id: Option<String>,
    curseforge_id: Option<String>,
) -> ModEntry {
    let display_name = followup
        .title
        .clone()
        .or_else(|| clean_string(&followup.display_name))
        .or_else(|| clean_string(followup.filename.trim_end_matches(".jar")))
        .unwrap_or_else(|| followup.key.clone());
    ModEntry {
        key: followup.key.clone(),
        filename: followup.filename.clone(),
        base: followup.filename.clone(),
        display_name,
        display_name_locked: true,
        installed_version: None,
        side: "universal".to_string(),
        library: false,
        technical: false,
        description: String::new(),
        dependencies: Vec::new(),
        resolved_dependencies: Vec::new(),
        jar_dependencies: Vec::new(),
        used_by: Vec::new(),
        cover_url: None,
        cover_path: None,
        cover_manual: false,
        cover_modified_at: None,
        source: followup.source.clone(),
        source_url: source_url(
            &followup.source,
            modrinth_id.as_deref(),
            followup.slug.as_deref(),
        ),
        has_index: false,
        has_tags: true,
        index_file: None,
        pack_side: None,
        modrinth_id,
        modrinth_version_id: None,
        curseforge_id,
        curseforge_file_id: None,
        duplicate: false,
        modified_at: now_iso(),
    }
}
