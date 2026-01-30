use std::collections::HashSet;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefaultWorktreeNameMode {
    Cities,
}

impl DefaultWorktreeNameMode {
    pub fn from_config(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "cities" => Some(Self::Cities),
            _ => None,
        }
    }
}

pub fn suggest_worktree_name(
    source_branch: &str,
    default_source: &str,
    mode: Option<DefaultWorktreeNameMode>,
    existing_names: &HashSet<String>,
) -> String {
    match mode {
        Some(DefaultWorktreeNameMode::Cities) => city_worktree_name(existing_names),
        None => branch_worktree_name(source_branch, default_source),
    }
}

pub fn city_worktree_name(existing_names: &HashSet<String>) -> String {
    let seed = random_seed();
    pick_city_name_with_seed(existing_names, seed)
}

fn branch_worktree_name(source_branch: &str, default_source: &str) -> String {
    if source_branch == default_source {
        return String::new();
    }

    source_branch
        .rsplit('/')
        .next()
        .unwrap_or(source_branch)
        .to_string()
}

fn pick_city_name_with_seed(existing_names: &HashSet<String>, seed: u64) -> String {
    if CITY_NAMES.is_empty() {
        return String::new();
    }

    let mut state = seed;
    let available: Vec<&'static str> = CITY_NAMES
        .iter()
        .copied()
        .filter(|name| !existing_names.contains(*name))
        .collect();

    if !available.is_empty() {
        let index = next_index(&mut state, available.len());
        return available[index].to_string();
    }

    let base_index = next_index(&mut state, CITY_NAMES.len());
    let base = CITY_NAMES[base_index];
    let mut suffix = 2;
    loop {
        let candidate = format!("{base}-{suffix}");
        if !existing_names.contains(&candidate) {
            return candidate;
        }
        suffix += 1;
    }
}

fn random_seed() -> u64 {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos() as u64)
        .unwrap_or(0);
    if nanos == 0 {
        0x9e37_79b9_7f4a_7c15
    } else {
        nanos
    }
}

fn next_index(state: &mut u64, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    let mut value = *state;
    if value == 0 {
        value = 0x9e37_79b9_7f4a_7c15;
    }
    value ^= value << 13;
    value ^= value >> 7;
    value ^= value << 17;
    *state = value;
    (value % len as u64) as usize
}

// Derived from Natural Earth populated places (public domain).
// Filtered to SCALERANK <= 4 and POP_MAX >= 1.2M, using NAME_EN/NAMEASCII.
// Then filtered to names with at most one hyphen and truncated to 250 entries.
const CITY_NAMES: &[&str] = &[
    "tokyo",
    "mexico-city",
    "mumbai",
    "sao-paulo",
    "delhi",
    "shanghai",
    "kolkata",
    "dhaka",
    "buenos-aires",
    "los-angeles",
    "karachi",
    "cairo",
    "osaka",
    "beijing",
    "manila",
    "moscow",
    "istanbul",
    "paris",
    "seoul",
    "lagos",
    "jakarta",
    "chicago",
    "guangzhou",
    "london",
    "lima",
    "tehran",
    "kinshasa",
    "bogota",
    "shenzhen",
    "wuhan",
    "hong-kong",
    "tianjin",
    "chennai",
    "taipei",
    "bengaluru",
    "bangkok",
    "lahore",
    "chongqing",
    "hyderabad",
    "amaravati",
    "santiago",
    "miami",
    "belo-horizonte",
    "madrid",
    "philadelphia",
    "ahmedabad",
    "toronto",
    "singapore",
    "luanda",
    "baghdad",
    "barcelona",
    "dallas",
    "shenyang",
    "khartoum",
    "pune",
    "sydney",
    "saint-petersburg",
    "chattogram",
    "dongguan",
    "atlanta",
    "boston",
    "riyadh",
    "houston",
    "hanoi",
    "washington",
    "guadalajara",
    "melbourne",
    "alexandria",
    "chengdu",
    "detroit",
    "yangon",
    "xi-an",
    "porto-alegre",
    "surat",
    "abidjan",
    "brasilia",
    "ankara",
    "monterrey",
    "nanjing",
    "montreal",
    "guiyang",
    "recife",
    "harbin",
    "fortaleza",
    "urumqi",
    "phoenix",
    "salvador",
    "busan",
    "san-francisco",
    "johannesburg",
    "berlin",
    "algiers",
    "rome",
    "pyongyang",
    "medellin",
    "kabul",
    "athens",
    "nagoya",
    "cape-town",
    "changchun",
    "casablanca",
    "dalian",
    "kanpur",
    "kano",
    "tel-aviv",
    "addis-ababa",
    "curitiba",
    "seattle",
    "zibo",
    "jeddah",
    "nairobi",
    "hangzhou",
    "caracas",
    "milan",
    "kunming",
    "jaipur",
    "san-diego",
    "taiyuan",
    "frankfurt",
    "qingdao",
    "surabaya",
    "lisbon",
    "jinan",
    "fukuoka",
    "campinas",
    "kaohsiung",
    "aleppo",
    "durban",
    "kyiv",
    "lucknow",
    "zhengzhou",
    "taichung",
    "ibadan",
    "minneapolis",
    "fuzhou",
    "dakar",
    "changsha",
    "izmir",
    "lanzhou",
    "incheon",
    "sapporo",
    "xiamen",
    "guayaquil",
    "george-town",
    "san-juan",
    "mashhad",
    "damascus",
    "nagpur",
    "lianshan",
    "shijiazhuang",
    "tunis",
    "vienna",
    "jilin-city",
    "omdurman",
    "bandung",
    "wenzhou",
    "nanchang",
    "tampa",
    "vancouver",
    "denver",
    "birmingham",
    "baltimore",
    "cali",
    "sendai",
    "naples",
    "manchester",
    "st-louis",
    "puebla-city",
    "tripoli",
    "tashkent",
    "nanchong",
    "havana",
    "nanning",
    "belem",
    "patna",
    "santo-domingo",
    "zaozhuang",
    "baku",
    "accra",
    "yantai",
    "medan",
    "xuzhou",
    "linyi",
    "maracaibo",
    "kuwait-city",
    "hiroshima",
    "baotou",
    "hefei",
    "indore",
    "goiania",
    "sanaa",
    "haiphong",
    "suzhou",
    "nanyang",
    "bucharest",
    "ningbo",
    "douala",
    "cleveland",
    "portland",
    "asuncion",
    "brisbane",
    "beirut",
    "pittsburgh",
    "las-vegas",
    "minsk",
    "kyoto",
    "barranquilla",
    "valencia",
    "hamburg",
    "vadodara",
    "manaus",
    "wuxi",
    "palembang",
    "san-bernardino",
    "brussels",
    "bhopal",
    "hohhot",
    "warsaw",
    "rabat",
    "quito",
    "antananarivo",
    "coimbatore",
    "daqing",
    "budapest",
    "san-jose",
    "ludhiana",
    "qiqihar",
    "anshan",
    "cincinnati",
    "handan",
    "isfahan",
    "yaounde",
    "sacramento",
    "shantou",
    "agra",
    "zhanjiang",
    "la-paz",
    "abuja",
    "harare",
    "tijuana",
    "khulna",
    "perth",
    "visakhapatnam",
    "multan",
    "kochi",
    "montevideo",
    "gujranwala",
    "florence",
    "conakry",
    "bamako",
];

#[cfg(test)]
mod tests {
    use super::{pick_city_name_with_seed, suggest_worktree_name, CITY_NAMES};
    use std::collections::HashSet;

    #[test]
    fn suggest_worktree_name_uses_branch_when_unset() {
        let existing = HashSet::new();
        let name = suggest_worktree_name("feature/thing", "origin/main", None, &existing);
        assert_eq!(name, "thing");
    }

    #[test]
    fn suggest_worktree_name_empty_for_default_source() {
        let existing = HashSet::new();
        let name = suggest_worktree_name("origin/main", "origin/main", None, &existing);
        assert!(name.is_empty());
    }

    #[test]
    fn pick_city_name_returns_available_city() {
        let mut existing: HashSet<String> = CITY_NAMES.iter().map(|name| name.to_string()).collect();
        existing.remove("london");
        let name = pick_city_name_with_seed(&existing, 1);
        assert_eq!(name, "london");
    }

    #[test]
    fn pick_city_name_appends_suffix_when_all_taken() {
        let existing: HashSet<String> = CITY_NAMES.iter().map(|name| name.to_string()).collect();
        let name = pick_city_name_with_seed(&existing, 2);
        assert!(name.ends_with("-2"));
        let base = name.strip_suffix("-2").expect("suffix");
        assert!(CITY_NAMES.contains(&base));
    }

    #[test]
    fn city_list_size_within_bounds() {
        assert_eq!(CITY_NAMES.len(), 250);
    }

    #[test]
    fn city_list_is_unique() {
        let mut seen = HashSet::new();
        for name in CITY_NAMES {
            assert!(seen.insert(*name), "duplicate city name {name}");
        }
    }

    #[test]
    fn city_list_has_sample_cities() {
        for city in ["tokyo", "london", "paris", "shanghai", "san-francisco"] {
            assert!(CITY_NAMES.contains(&city), "missing {city}");
        }
    }

    #[test]
    fn city_list_uses_slug_format() {
        for name in CITY_NAMES {
            assert!(!name.is_empty());
            assert!(!name.starts_with('-'));
            assert!(!name.ends_with('-'));
            assert!(!name.contains("--"));
            assert!(name
                .chars()
                .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-'));
            assert!(name.matches('-').count() <= 1);
        }
    }
}
