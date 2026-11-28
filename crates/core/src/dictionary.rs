/// Offline paper-margin literary dictionary for instant, zero-latency footnote lookups.
/// Covers classical Chinese literary terms, idioms, and reflective English reading vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DictEntry {
    pub headword: &'static str,
    pub reading: &'static str,
    pub brief: &'static str,
    pub literary_note: &'static str,
}

const ENTRIES: &[DictEntry] = &[
    // Classical Chinese & literary terms
    DictEntry {
        headword: "豁然开朗",
        reading: "huò rán kāi lǎng",
        brief: "从狭窄幽暗处忽然步入开阔明亮的境地；也形容思绪陡然贯通。",
        literary_note: "语出陶渊明《桃花源记》：“复行数十步，豁然开朗。”",
    },
    DictEntry {
        headword: "落英缤纷",
        reading: "luò yīng bīn fēn",
        brief: "坠落的花瓣繁多而纷扬，色彩柔和错落。",
        literary_note: "“英”指花朵，句中既有花瓣飘落之态，亦有春林静谧之气。",
    },
    DictEntry {
        headword: "怡然自乐",
        reading: "yí rán zì lè",
        brief: "安适愉快、自得其乐，不依赖外界喧哗。",
        literary_note: "“怡然”指心神舒缓平和的喜悦。",
    },
    DictEntry {
        headword: "黄发垂髫",
        reading: "huáng fà chuí tiáo",
        brief: "借指老人与孩童。",
        literary_note: "古人称高龄老人发白转黄为“黄发”，孩童垂下的额发为“垂髫”。",
    },
    DictEntry {
        headword: "阡陌交通",
        reading: "qiān mò jiāo tōng",
        brief: "田间南北与东西走向的小路纵横相通。",
        literary_note: "南北为“阡”，东西为“陌”，此处写乡野安宁互通的景象。",
    },
    DictEntry {
        headword: "心为形役",
        reading: "xīn wéi xíng yì",
        brief: "内心被外在形体、生计或世俗牵绊所驱使。",
        literary_note: "出自《归去来兮辞》，是陶渊明回望仕途、决定顺从本心的转折句。",
    },
    DictEntry {
        headword: "熹微",
        reading: "xī wēi",
        brief: "清晨天光初现、柔和微弱的样子。",
        literary_note: "“恨晨光之熹微”以微光映衬归家途中的急切与柔情。",
    },
    DictEntry {
        headword: "出岫",
        reading: "chū xiù",
        brief: "云雾自然地从山谷岩穴间飘散而出。",
        literary_note: "“云无心以出岫，鸟倦飞而知还”比喻顺应本性、不执着于进退。",
    },
    DictEntry {
        headword: "盘桓",
        reading: "pán huán",
        brief: "流连徘徊、不忍遽去。",
        literary_note: "“抚孤松而盘桓”写人在暮色中轻抚青松的从容驻足。",
    },
    DictEntry {
        headword: "既望",
        reading: "jì wàng",
        brief: "农历每月十六日，即满月（望日）之后的第一天。",
        literary_note: "农历十五称“望”，十六称“既望”，月色依然饱满清亮。",
    },
    DictEntry {
        headword: "清风徐来",
        reading: "qīng fēng xú lái",
        brief: "清爽的晚风缓缓拂过。",
        literary_note: "《前赤壁赋》开篇八字“清风徐来，水波不兴”定下整篇的澄澈基调。",
    },
    DictEntry {
        headword: "羽化而登仙",
        reading: "yǔ huà ér dēng xiān",
        brief: "轻盈飘然、仿佛脱去尘累而凌空飞升。",
        literary_note: "苏轼以极轻的意象写出江上泛舟时的身心舒展。",
    },
    DictEntry {
        headword: "愀然",
        reading: "qiǎo rán",
        brief: "神色变得凝重、肃然或略带忧思。",
        literary_note: "常用于情绪从欢畅转向沉思的叙事转折处。",
    },
    DictEntry {
        headword: "扁舟",
        reading: "piān zhōu",
        brief: "轻盈小巧的一叶小船（此处“扁”读 piān）。",
        literary_note: "中国古典文学中常象征自由、远游与江湖淡泊。",
    },
    DictEntry {
        headword: "沧海一粟",
        reading: "cāng hǎi yī sù",
        brief: "大海中的一粒小米，喻个体在浩瀚时空里的渺小。",
        literary_note: "苏轼先言“渺沧海之一粟”，继而转入“物与我皆无尽也”的旷达。",
    },
    DictEntry {
        headword: "须臾",
        reading: "xū yú",
        brief: "极短的片刻时间。",
        literary_note: "“哀吾生之须臾，羡长江之无穷”对比短暂人生与永恒江流。",
    },
    DictEntry {
        headword: "无尽藏",
        reading: "wú jìn zàng",
        brief: "取用不竭的宝藏（此处“藏”读 zàng）。",
        literary_note: "原为佛家语，苏轼借指江上清风与山间明月这类不需占有的丰饶之美。",
    },
    DictEntry {
        headword: "藻荇交横",
        reading: "zǎo xìng jiāo héng",
        brief: "水草纵横交错的样子。",
        literary_note: "《记承天寺夜游》借水中藻荇喻月下竹柏疏影，全篇不著一“月”字而月色满庭。",
    },
    DictEntry {
        headword: "闲人",
        reading: "xián rén",
        brief: "不急于奔忙逐利、仍有余暇欣赏月色与草木的人。",
        literary_note: "在苏轼笔下，“闲人”并非贬义，而是保有清静心境的温柔自嘲。",
    },
    DictEntry {
        headword: "竹杖芒鞋",
        reading: "zhú zhàng máng xié",
        brief: "竹子手杖与草编凉鞋，指轻便朴素的行装。",
        literary_note: "即便遇雨狼狈，手中一杖、脚下一履仍能步履轻快。",
    },
    DictEntry {
        headword: "一蓑烟雨",
        reading: "yī suō yān yǔ",
        brief: "披着一件蓑衣在烟雨中从容前行，喻坦然面对人生起伏。",
        literary_note: "“一蓑烟雨任平生”是东坡面对逆境最具治愈力的一句词。",
    },
    DictEntry {
        headword: "料峭",
        reading: "liào qiào",
        brief: "形容早春微寒、带着清冽寒意的风。",
        literary_note: "“料峭春风吹酒醒”以微冷反衬后文“山头斜照却相迎”的暖意。",
    },
    DictEntry {
        headword: "萧瑟",
        reading: "xiāo sè",
        brief: "草木摇落、风声凄清的样子；亦指经历过的低谷处境。",
        literary_note: "回头看刚刚走过的风雨处，心定之后便“也无风雨也无晴”。",
    },
    DictEntry {
        headword: "营营",
        reading: "yíng yíng",
        brief: "为了世俗得失而往来奔波、劳神不息的样子。",
        literary_note: "“何时忘却营营”道出每个疲惫夜晚想放下执念的心愿。",
    },
    DictEntry {
        headword: "縠纹",
        reading: "hú wén",
        brief: "如轻纱绉绸般细密柔软的水面波纹。",
        literary_note: "“夜阑风静縠纹平”写深夜风止水静，心境亦随之归于平缓。",
    },
    DictEntry {
        headword: "秋暝",
        reading: "qiū míng",
        brief: "秋日的傍晚时分，“暝”指日暮天色渐暗。",
        literary_note: "王维以雨后薄暮写山居，明暗冷暖交织成一幅水墨画。",
    },
    DictEntry {
        headword: "行到水穷处",
        reading: "xíng dào shuǐ qióng chù",
        brief: "漫步走到溪流尽头，不再焦虑无路可走。",
        literary_note: "下句“坐看云起时”展现随遇而安、在停顿处看见新风景的禅意。",
    },
    DictEntry {
        headword: "幽篁",
        reading: "yōu huáng",
        brief: "幽静深密的竹林。",
        literary_note: "“独坐幽篁里”以青翠安静的竹林隔开尘嚣。",
    },
    DictEntry {
        headword: "返景",
        reading: "fǎn yǐng",
        brief: "傍晚夕阳西斜时回照入林的余晖（“景”同“影”，指日光）。",
        literary_note: "出自王维《鹿柴》，一束斜阳照在青苔上，反衬深林极静。",
    },
    DictEntry {
        headword: "依依",
        reading: "yī yī",
        brief: "柔和轻缓、眷恋不舍或亲切从容的神态。",
        literary_note: "“相见语依依”写乡邻傍晚归途上不紧不慢的闲话家常。",
    },
    DictEntry {
        headword: "留白",
        reading: "liú bái",
        brief: "刻意保留空白与停顿，让视线与思绪得以呼吸。",
        literary_note: "书画与生活皆贵在留白——不必把每一分钟都填满。",
    },
    DictEntry {
        headword: "怅然",
        reading: "chàng rán",
        brief: "若有所失、微微惆怅而不激烈的失落感。",
        literary_note: "多指触景生情时的片刻出神。",
    },
    DictEntry {
        headword: "氤氲",
        reading: "yīn yūn",
        brief: "烟气、雾霭或茶香轻轻弥漫萦绕的样子。",
        literary_note: "常用来描绘雨夜灯光、热茶水汽与山间薄雾的柔和质感。",
    },
    DictEntry {
        headword: "阑珊",
        reading: "lán shān",
        brief: "将尽、渐稀或灯光零落微弱的样子。",
        literary_note: "如“灯火阑珊处”，指喧闹退去之后仍安静亮着的一盏灯。",
    },
    DictEntry {
        headword: "羁旅",
        reading: "jī lǚ",
        brief: "长期旅居异乡、漂泊在外的生活状态。",
        literary_note: "古典诗词常见主题，常寄托思乡与自我安顿之情。",
    },
    DictEntry {
        headword: "逍遥",
        reading: "xiāo yáo",
        brief: "悠然自得、不受拘束的精神舒展。",
        literary_note: "源自《庄子·逍遥游》，重在内心通透而非外物占有。",
    },
    // Reflective English literary vocabulary
    DictEntry {
        headword: "deliberately",
        reading: "/dɪˈlɪb.ə.rət.li/",
        brief: "从容而清醒地；带着觉察、有意识地去生活或行动。",
        literary_note: "Thoreau wrote 'to live deliberately'—choosing presence over rush.",
    },
    DictEntry {
        headword: "solitude",
        reading: "/ˈsɒl.ɪ.tjuːd/",
        brief: "丰盈而安宁的独处（不同于感到匮乏的孤独 loneliness）。",
        literary_note: "在文学中常指向与自己温柔相处的安静时光。",
    },
    DictEntry {
        headword: "resignation",
        reading: "/ˌrez.ɪɡˈneɪ.ʃən/",
        brief: "无奈的顺从、听天由命地放弃抵抗。",
        literary_note: "梭罗在《瓦尔登湖》中说他不想过毫无生气的“凑合人生”。",
    },
    DictEntry {
        headword: "marrow",
        reading: "/ˈmær.əʊ/",
        brief: "精髓、核心滋养（本义为骨髓）。",
        literary_note: "'suck out all the marrow of life' 意为深深品味生活最本真的滋味。",
    },
    DictEntry {
        headword: "wholesome",
        reading: "/ˈhəʊl.səm/",
        brief: "有益身心健康、使人清爽安稳的。",
        literary_note: "梭罗称大部分时间的独处对心神而言是 wholesome 的。",
    },
    DictEntry {
        headword: "wearisome",
        reading: "/ˈwɪə.ri.səm/",
        brief: "令人疲倦乏力、耗散心神的。",
        literary_note: "形容即便身处人群，过度社交也会悄然消耗安静的能量。",
    },
    DictEntry {
        headword: "melancholy",
        reading: "/ˈmel.ən.kɒl.i/",
        brief: "深沉而安静的忧郁、淡淡的哀愁。",
        literary_note: "古典文学常将其视作沉思与敏感心灵的底色。",
    },
    DictEntry {
        headword: "imbibes",
        reading: "/ɪmˈbaɪbz/",
        brief: "深深汲取、仿佛每一寸感官都在吸收滋养。",
        literary_note: "'imbibes delight through every pore' 描绘黄昏漫步时通透的愉悦。",
    },
    DictEntry {
        headword: "vagrants",
        reading: "/ˈveɪ.ɡrənts/",
        brief: "自由漂泊的旅人、没有固定归处的漫游者。",
        literary_note: "泰戈尔用 'little vagrants' 温柔地称呼风、落叶与掠过窗前的飞鸟。",
    },
    DictEntry {
        headword: "constancy",
        reading: "/ˈkɒn.stən.si/",
        brief: "恒久不变的坚守、安静而长情的陪伴。",
        literary_note: "出自《飞鸟集》：灯座在阴影里以 constancy 托举着光明。",
    },
    DictEntry {
        headword: "ephemeral",
        reading: "/ɪˈfem.ər.əl/",
        brief: "朝生暮死般的、转瞬即逝却动人的。",
        literary_note: "常用来形容黄昏云彩、花期与某个值得珍惜的阅读片刻。",
    },
    DictEntry {
        headword: "serene",
        reading: "/səˈriːn/",
        brief: "澄澈宁静、波澜不惊的。",
        literary_note: "带有天空放晴、湖面平整般的安抚质感。",
    },
    DictEntry {
        headword: "luminous",
        reading: "/ˈluː.mɪ.nəs/",
        brief: "散发柔和微光的、清亮而通透的。",
        literary_note: "既可形容月光与灯晕，也可形容一段清晰动人的文字。",
    },
    DictEntry {
        headword: "sanctuary",
        reading: "/ˈsæŋk.tʃu.ə.ri/",
        brief: "庇护所、心灵可以安全停泊的静谧空间。",
        literary_note: "书房与书页，正是繁忙世界之外随手可及的 sanctuary。",
    },
    // Expanded contemplative & healing literary entries
    DictEntry {
        headword: "静谧",
        reading: "jìng mì",
        brief: "安宁清净、没有丝毫躁扰的柔和安静。",
        literary_note: "比单纯的无声多了一层温润的包容感，常形容夜色与书房。",
    },
    DictEntry {
        headword: "余韵",
        reading: "yú yùn",
        brief: "乐声、茶香或好句子在停顿之后仍萦绕心间的悠长回味。",
        literary_note: "合上书页后的片刻出神，正是文字在心里留下的余韵。",
    },
    DictEntry {
        headword: "清欢",
        reading: "qīng huān",
        brief: "淡雅素净、不借外物喧闹的细微欢喜。",
        literary_note: "苏轼有“人间有味是清欢”，指一壶清茶、几页闲书中的安然。",
    },
    DictEntry {
        headword: "素履",
        reading: "sù lǚ",
        brief: "朴素无华的鞋履，常喻保持本心、不炫耀也不急躁地前行。",
        literary_note: "“素履以往”意为带着简单清明的初心，安稳走自己的路。",
    },
    DictEntry {
        headword: "松弛",
        reading: "sōng chí",
        brief: "卸下紧绷与戒备，让身体与心绪回到舒展自然的节律。",
        literary_note: "并非懈怠，而是允许自己像拉满后轻轻放下的弓弦一样休整。",
    },
    DictEntry {
        headword: "微澜",
        reading: "wēi lán",
        brief: "水面上轻柔细小的波纹，也指心中泛起的细微触动。",
        literary_note: "读书时偶遇一句知心话，心湖便会轻轻荡开一阵微澜。",
    },
    DictEntry {
        headword: "澄澈",
        reading: "chéng chè",
        brief: "水清见底、通透无浊，亦形容思绪明朗清净。",
        literary_note: "常用于雨过天晴、情绪沉淀之后的清明状态。",
    },
    DictEntry {
        headword: "栖息",
        reading: "qī xī",
        brief: "飞鸟归林停歇，引申为疲惫的心灵找到安心歇脚之处。",
        literary_note: "每天睡前的十分钟阅读，就是让思绪安稳栖息的枝头。",
    },
    DictEntry {
        headword: "片刻",
        reading: "piàn kè",
        brief: "短暂却完整的一小段时间。",
        literary_note: "哪怕一天只有片刻属于自己，也足够把心重新安放稳当。",
    },
    DictEntry {
        headword: "归途",
        reading: "guī tú",
        brief: "回家的路，也指从外界喧嚣回到内在安宁的过程。",
        literary_note: "文学中常以归途映照卸下防备、找回自我的瞬间。",
    },
    DictEntry {
        headword: "灯火",
        reading: "dēng huǒ",
        brief: "夜色里温暖亮着的灯光，象征人间烟火与长情守候。",
        literary_note: "桌前的一盏台灯，就是夜晚最温柔的坐标。",
    },
    DictEntry {
        headword: "远行",
        reading: "yuǎn xíng",
        brief: "离开熟悉原地去往更辽阔之处，也指借由书页展开的精神漫游。",
        literary_note: "身体留在一盏灯下，思绪却能随文字完成一场安静的远行。",
    },
    DictEntry {
        headword: "潮汐",
        reading: "cháo xī",
        brief: "海水有节律的涨落，常喻情绪与精力自然的起伏周期。",
        literary_note: "理解情绪如潮汐，便不再为低落时的退潮感到自责。",
    },
    DictEntry {
        headword: "tranquility",
        reading: "/træŋˈkwɪl.ə.ti/",
        brief: "宁静致远的平和心境，不受外界浮躁惊扰。",
        literary_note: "常与静湖、晚风与独处书房相伴出现。",
    },
    DictEntry {
        headword: "stillness",
        reading: "/ˈstɪl.nəs/",
        brief: "万籁俱寂时的静止与沉定。",
        literary_note: "在 stillness 之中，细小的灵感与内在声音往往更容易浮现。",
    },
    DictEntry {
        headword: "reverie",
        reading: "/ˈrev.ər.i/",
        brief: "温柔的白日梦、沉醉其中的出神遐想。",
        literary_note: "雨天望向窗外或在书页间短暂停顿，便是一场轻盈的 reverie。",
    },
    DictEntry {
        headword: "equanimity",
        reading: "/ˌek.wəˈnɪm.ə.ti/",
        brief: "面对顺逆境遇仍能保持的从容定力与内心的平稳。",
        literary_note: "与苏轼“也无风雨也无晴”的旷达心境遥相呼应。",
    },
    DictEntry {
        headword: "impermanence",
        reading: "/ɪmˈpɜː.mə.nəns/",
        brief: "无常；万物不断流转变化的本质。",
        literary_note: "看见无常既让人珍惜眼前一页微光，也让人相信低谷终会过去。",
    },
    DictEntry {
        headword: "serenity",
        reading: "/səˈren.ə.ti/",
        brief: "澄净安详、云淡风轻的内在安定。",
        literary_note: "带有被温柔接住后的明朗平静。",
    },
    DictEntry {
        headword: "resilience",
        reading: "/rɪˈzɪl.jəns/",
        brief: "韧性；经历弯曲或疲惫之后，慢慢恢复舒展的力量。",
        literary_note: "真正的 resilience 不是硬撑，而是允许自己先休息再出发。",
    },
    DictEntry {
        headword: "twilight",
        reading: "/ˈtwaɪ.laɪt/",
        brief: "黄昏薄暮或拂晓微明，昼夜柔和交接的过渡时刻。",
        literary_note: "蓝调时刻的天色与台灯微光相遇，是极适合慢读的时分。",
    },
    DictEntry {
        headword: "lullaby",
        reading: "/ˈlʌl.ə.baɪ/",
        brief: "摇篮曲；任何轻柔缓慢、能安抚入眠的声音或文字节律。",
        literary_note: "细雨声与舒缓的朗读语速，都是写给成年人的温柔 lullaby。",
    },
    DictEntry {
        headword: "lantern",
        reading: "/ˈlæn.tən/",
        brief: "提灯、暖光灯笼；在暗处护住一小片光亮与方向的器物。",
        literary_note: "提一盏灯不必照亮整座山林，照见眼前一步就足以安心。",
    },
    DictEntry {
        headword: "haven",
        reading: "/ˈheɪ.vən/",
        brief: "避风港、可以安心歇脚的温柔所在。",
        literary_note: "愿每一座本地书房都成为远离信息焦虑的 quiet haven。",
    },
    DictEntry {
        headword: "solace",
        reading: "/ˈsɒl.əs/",
        brief: "慰藉；在疲惫或失落时轻轻托住情绪的温暖安慰。",
        literary_note: "一句恰好读懂此刻心情的文字，胜过千言说教的 solace。",
    },
    DictEntry {
        headword: "quietude",
        reading: "/ˈkwaɪ.ə.tjuːd/",
        brief: "清静恬淡的状态，偏重内心远离纷扰后的悠然。",
        literary_note: "比 quiet 更具文学气息，常形容长夜独坐读书的氛围。",
    },
];

pub fn entry_count() -> usize {
    ENTRIES.len()
}

/// Looks up a selected word or short passage in the offline paper-margin literary dictionary.
pub fn lookup(selection: &str) -> Option<DictEntry> {
    let cleaned = clean_selection(selection);
    if cleaned.is_empty() {
        return None;
    }
    let lower = cleaned.to_ascii_lowercase();

    // 1. Exact match first
    if let Some(entry) = ENTRIES
        .iter()
        .find(|e| e.headword == cleaned || e.headword.eq_ignore_ascii_case(&lower))
    {
        return Some(*entry);
    }

    // 2. Selection contains a known headword (prefer the longest headword match)
    let mut best: Option<DictEntry> = None;
    for entry in ENTRIES {
        let hit = if entry.headword.is_ascii() {
            lower
                .split(|c: char| !c.is_ascii_alphabetic())
                .any(|word| word == entry.headword || word.starts_with(entry.headword))
        } else {
            cleaned.contains(entry.headword)
        };
        if hit {
            let better = best.is_none_or(|prev| entry.headword.len() > prev.headword.len());
            if better {
                best = Some(*entry);
            }
        }
    }
    if best.is_some() {
        return best;
    }

    // 3. Headword contains a >=2-char CJK or >=4-char English query
    if cleaned.chars().count() >= 2 {
        return ENTRIES
            .iter()
            .find(|e| {
                if e.headword.is_ascii() {
                    lower.len() >= 4 && e.headword.to_ascii_lowercase().starts_with(&lower)
                } else {
                    e.headword.contains(&cleaned)
                }
            })
            .copied();
    }
    None
}

fn clean_selection(raw: &str) -> String {
    raw.trim()
        .trim_matches(|c: char| {
            c.is_whitespace()
                || matches!(
                    c,
                    '，' | '。'
                        | '！'
                        | '？'
                        | '、'
                        | '；'
                        | '：'
                        | '“'
                        | '”'
                        | '‘'
                        | '’'
                        | '《'
                        | '》'
                        | '（'
                        | '）'
                        | ','
                        | '.'
                        | '!'
                        | '?'
                        | ';'
                        | ':'
                        | '"'
                        | '\''
                        | '('
                        | ')'
                )
        })
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_exact_phrase_and_english_literary_footnotes_offline() {
        assert!(entry_count() >= 75);
        let exact = lookup("豁然开朗。").unwrap();
        assert_eq!(exact.headword, "豁然开朗");
        assert!(exact.literary_note.contains("桃花源记"));

        let phrase = lookup("一蓑烟雨任平生").unwrap();
        assert_eq!(phrase.headword, "一蓑烟雨");

        let en = lookup("I love solitude,").unwrap();
        assert_eq!(en.headword, "solitude");
        assert!(en.brief.contains("独处"));

        assert_eq!(lookup("人间有味是清欢").unwrap().headword, "清欢");
        assert_eq!(lookup("文字留下的余韵").unwrap().headword, "余韵");
        assert_eq!(lookup("quiet haven tonight").unwrap().headword, "haven");

        assert!(lookup("   ").is_none());
    }
}
