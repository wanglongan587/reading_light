use crate::{Store, library::text_epub};
use anyhow::{Context, Result};

/// Built-in curated public-domain & original reading shelf ("微光公版馆").
/// Every title here is either original to Glow Reading or verified public domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CatalogBook {
    pub id: &'static str,
    pub title: &'static str,
    pub author: &'static str,
    pub category: &'static str,
    pub era: &'static str,
    pub minutes: u32,
    pub moods: &'static [&'static str],
    pub tagline: &'static str,
    pub accent_hex: &'static str,
    pub content: &'static str,
}

const SAMPLE_FOREST: &str = include_str!("../../../assets/sample.txt");

const TAOYUANMING_TEXT: &str = r#"# 桃花源记
晋太元中，武陵人捕鱼为业。缘溪行，忘路之远近。忽逢桃花林，夹岸数百步，中无杂树，芳草鲜美，落英缤纷。渔人甚异之，复前行，欲穷其林。
林尽水源，便得一山，山有小口，仿佛若有光。便舍船，从口入。初极狭，才通人。复行数十步，豁然开朗。土地平旷，屋舍俨然，有良田、美池、桑竹之属。阡陌交通，鸡犬相闻。其中往来种作，男女衣着，悉如外人。黄发垂髫，并怡然自乐。
见渔人，乃大惊，问所从来。具答之。便要还家，设酒杀鸡作食。村中闻有此人，咸来问讯。自云先世避秦时乱，率妻子邑人来此绝境，不复出焉，遂与外人间隔。问今是何世，乃不知有汉，无论魏晋。此人一一为具言所闻，皆叹惋。余人各复延至其家，皆出酒食。停数日，辞去。此中人语云：“不足为外人道也。”
既出，得其船，便扶向路，处处志之。及郡下，诣太守，说如此。太守即遣人随其往，寻向所志，遂迷，不复得路。
南阳刘子骥，高尚士也，闻之，欣然规往。未果，寻病终，后遂无问津者。

# 归去来兮辞
归去来兮，田园将芜胡不归！既自以心为形役，奚惆怅而独悲？悟已往之不谏，知来者之可追。实迷途其未远，觉今是而昨非。舟遥遥以轻飏，风飘飘而吹衣。问征夫以前路，恨晨光之熹微。
乃瞻衡宇，载欣载奔。僮仆欢迎，稚子候门。三径就荒，松菊犹存。携幼入室，有酒盈樽。引壶觞以自酌，眄庭柯以怡颜。倚南窗以寄傲，审容膝之易安。园日涉以成趣，门虽设而常关。策扶老以流憩，时矫首而遐观。云无心以出岫，鸟倦飞而知还。景翳翳以将入，抚孤松而盘桓。
归去来兮，请息交以绝游。世与我而相违，复驾言兮焉求？悦亲戚之情话，乐琴书以消忧。农人告余以春及，将有事于西畴。或命巾车，或棹孤舟。既窈窕以寻壑，亦崎岖而经丘。木欣欣以向荣，泉涓涓而始流。善万物之得时，感吾生之行休。
已矣乎！寓形宇内复几时？曷不委心任去留？胡为乎遑遑欲何之？富贵非吾愿，帝乡不可期。怀良辰以孤往，或植杖而耘耔。登东皋以舒啸，临清流而赋诗。聊乘化以归尽，乐夫天命复奚疑！

# 饮酒 · 其五
结庐在人境，而无车马喧。
问君何能尔？心远地自偏。
采菊东篱下，悠然见南山。
山气日夕佳，飞鸟相与还。
此中有真意，欲辨已忘言。
"#;

const SUSHI_TEXT: &str = r#"# 前赤壁赋
壬戌之秋，七月既望，苏子与客泛舟游于赤壁之下。清风徐来，水波不兴。举酒属客，诵明月之诗，歌窈窕之章。少焉，月出于东山之上，徘徊于斗牛之间。白露横江，水光接天。纵一苇之所如，凌万顷之茫然。浩浩乎如冯虚御风，而不知其所止；飘飘乎如遗世独立，羽化而登仙。
于是饮酒乐甚，扣舷而歌之。歌曰：“桂棹兮兰桨，击空明兮溯流光。渺渺兮予怀，望美人兮天一方。”客有吹洞箫者，倚歌而和之。其声呜呜然，如怨如慕，如泣如诉，余音袅袅，不绝如缕。舞幽壑之潜蛟，泣孤舟之嫠妇。
苏子愀然，正襟危坐而问客曰：“何为其然也？”客曰：“‘月明星稀，乌鹊南飞’，此非曹孟德之诗乎？西望夏口，东望武昌，山川相缪，郁乎苍苍，此非孟德之困于周郎者乎？方其破荆州，下江陵，顺流而东也，舳舻千里，旌旗蔽空，酾酒临江，横槊赋诗，固一世之雄也，而今安在哉？况吾与子渔樵于江渚之上，侣鱼虾而友麋鹿，驾一叶之扁舟，举匏樽以相属。寄蜉蝣于天地，渺沧海之一粟。哀吾生之须臾，羡长江之无穷。挟飞仙以遨游，抱明月而长终。知不可乎骤得，托遗响于悲风。”
苏子曰：“客亦知夫水与月乎？逝者如斯，而未尝往也；盈虚者如彼，而卒莫消长也。盖将自其变者而观之，则天地曾不能以一瞬；自其不变者而观之，则物与我皆无尽也，而又何羡乎！且夫天地之间，物各有主，苟非吾之所有，虽一毫而莫取。惟江上之清风，与山间之明月，耳得之而为声，目遇之而成色，取之无禁，用之不竭，是造物者之无尽藏也，而吾与子之所共适。”
客喜而笑，洗盏更酌。肴核既尽，杯盘狼籍。相与枕藉乎舟中，不知东方之既白。

# 记承天寺夜游
元丰六年十月十二日夜，解衣欲睡，月色入户，欣然起行。念无与为乐者，遂至承天寺寻张怀民。怀民亦未寝，相与步于中庭。庭下如积水空明，水中藻、荇交横，盖竹柏影也。何夜无月？何处无竹柏？但少闲人如吾两人者耳。

# 定风波 · 莫听穿林打叶声
三月七日，沙湖道中遇雨，雨具先去，同行皆狼狈，余独不觉。已而遂晴，故作此词。
莫听穿林打叶声，何妨吟啸且徐行。竹杖芒鞋轻胜马，谁怕？一蓑烟雨任平生。
料峭春风吹酒醒，微冷，山头斜照却相迎。回首向来萧瑟处，归去，也无风雨也无晴。

# 临江仙 · 夜饮东坡醒复醉
夜饮东坡醒复醉，归来仿佛三更。家童鼻息已雷鸣。敲门都不应，倚杖听江声。
长恨此身非我有，何时忘却营营。夜阑风静縠纹平。小舟从此逝，江海寄余生。
"#;

const WANGWEI_TEXT: &str = r#"# 山居秋暝
空山新雨后，天气晚来秋。
明月松间照，清泉石上流。
竹喧归浣女，莲动下渔舟。
随意春芳歇，王孙自可留。

# 终南别业
中岁颇好道，晚家南山陲。
兴来每独往，胜事空自知。
行到水穷处，坐看云起时。
偶然值林叟，谈笑无还期。

# 竹里馆
独坐幽篁里，弹琴复长啸。
深林人不知，明月来相照。

# 鹿柴
空山不见人，但闻人语响。
返景入深林，复照青苔上。

# 鸟鸣涧
人闲桂花落，夜静春山空。
月出惊山鸟，时鸣春涧中。

# 辛夷坞
木末芙蓉花，山中发红萼。
涧户寂无人，纷纷开且落。

# 渭川田家
斜阳照墟落，穷巷牛羊归。
野老念牧童，倚杖候荆扉。
雉雊麦苗秀，蚕眠桑叶稀。
田夫荷锄至，相见语依依。
即此羡闲逸，怅然吟式微。
"#;

const WALDEN_TEXT: &str = r#"# Where I Lived, and What I Lived For
I went to the woods because I wished to live deliberately, to front only the essential facts of life, and see if I could not learn what it had to teach, and not, when I came to die, discover that I had not lived.
I did not wish to live what was not life, living is so dear; nor did I wish to practise resignation, unless it was quite necessary. I wanted to live deep and suck out all the marrow of life, to live so sturdily and Spartan-like as to put to rout all that was not life.
Morning is when I am awake and there is a dawn in me. Moral reform is the effort to throw off sleep. To be awake is to be alive. I have never yet met a man who was quite awake.
Time is but the stream I go a-fishing in. I drink at it; but while I drink I see the sandy bottom and detect how shallow it is. Its thin current slides away, but eternity remains.

# Solitude
This is a delicious evening, when the whole body is one sense, and imbibes delight through every pore. I go and come with a strange liberty in Nature, a part of herself.
I find it wholesome to be alone the greater part of the time. To be in company, even with the best, is soon wearisome and dissipating. I love to be alone. I never found the companion that was so companionable as solitude.
We are for the most part more lonely when we go abroad among men than when we stay in our chambers. A man thinking or working is always alone, let him be where he will. Solitude is not measured by the miles of space that intervene between a man and his fellows.
There can be no very black melancholy to him who lives in the midst of Nature and has his senses still. While I enjoy the friendship of the seasons I trust that nothing can make life a burden to me.
"#;

const STRAY_BIRDS_TEXT: &str = r#"# 飞鸟集 · 选札一
Stray birds of summer come to my window to sing and fly away. And yellow leaves of autumn, which have no songs, flutter and fall there with a sigh.
夏天的飞鸟，飞到我窗前唱歌，又飞去了。秋天的黄叶，它们没有什么可唱，只叹息一声，飞落在那里。
O troupe of little vagrants of the world, leave your footprints in my words.
世界上的一队小小的漂泊者呀，请留下你们的足印在我的文字里。
The world puts off its mask of vastness to its lover. It becomes small as one song, as one kiss of the eternal.
世界对着它的爱人，把它浩瀚的面具揭下了。它变小了，小如一首歌，小如一回永恒的接吻。
If you shed tears when you miss the sun, you also miss the stars.
如果你因失去了太阳而流泪，那么你也将失去群星了。
The waterfall sings, "I find my song, when I find my freedom."
瀑布歌唱道：“我得到我的自由时，我便有了歌声。”

# 飞鸟集 · 选札二
Let life be beautiful like summer flowers and death like autumn leaves.
生如夏花之绚烂，死如秋叶之静美。
Listen, my heart, to the whispers of the world with which it makes love to you.
静静地听，我的心呀，听那世界的低语，这是它对你求爱的表示。
The noise of the moment scoffs at the music of the Eternal.
片刻的喧嚣，嘲笑着永恒的音乐。
I cannot choose the best. The best chooses me.
我不能选择那最好的。是那最好的选择我。
God finds himself by creating.
创造让世界照见自己。
Thank the flame for its light, but do not forget the lampholder standing in the shade with constancy of patience.
感谢火焰给你光明，但是不要忘了那执灯的人，他是坚忍地站在黑暗当中的。
"#;

const STARLIGHT_POST_TEXT: &str = r#"# 第一夜 · 山腰上的慢邮局
从镇口往北走，越过三座长满青苔的石拱桥，就到了只在傍晚六点以后亮灯的慢邮局。邮局没有加急件，也没有催促人的叫号屏幕，木门上只挂着一枚小小的黄铜风铃。

邮差阿禾喜欢在开门前先擦亮柜台上的玻璃台灯。灯光是浅浅的琥珀色，照在旧牛皮纸信封上，像把傍晚最后一缕斜阳叠进了抽屉里。

“今天也辛苦啦。”每当有人推门进来，阿禾总是先递上一小杯温热的陈皮水，再不紧不慢地问对方想把这封信寄给哪一天的自己。

# 第二夜 · 迟到也无妨的回信
有一位总在深夜加班的绘本画师，连续三个月把没画完的草稿折成小船，投进慢邮局的蓝色邮筒里。他在信封角落写着：“如果明天我还是画不好，就替我把这些纸船收起来吧。”

秋分那天，阿禾把一叠压平整的草稿装进新的信袋，贴上一枚印着萤火虫的邮票，轻轻放回了画师家门口的信箱。信里没有一句大道理，只夹着一片银杏叶和一行细小的钢笔字：

“风停的时候，纸船也会稳稳停在水面上。你已经走了很远的路，今晚先安心睡一觉。”

# 第三夜 · 留给明天的一盏灯
冬天落第一场小雪时，慢邮局的屋檐垂下了一排晶莹的冰凌。阿禾把柜台里的每盏小台灯都调暗了一格，让屋子安静得能听见雪花落在松针上的微响。

他合上今日的登记簿，在最后一页记下：今天共有七个人停下来喘了口气，三个人原谅了自己没做完的计划，还有一个人决定把手机放到客厅，安安静静读完十页书再睡。

推开窗，远处的山谷像一册合拢的旧画集。灯光不必照亮整片原野，只要足够照见眼前这一页，就已经很好了。
"#;

const RAINY_TEA_NOTES_TEXT: &str = r#"# 一 · 听雨之前，先烧一壶水
傍晚下起细雨时，房间里最先安静下来的往往不是街声，而是水壶刚坐上炉子时那阵低沉的嗡鸣。气泡从壶底一颗颗浮起来，像把白天散落各处的念头重新聚拢。

泡一盏温润的焙火乌龙，不必讲究繁复的器皿。一只顺手的粗陶杯，一撮带着淡淡焦糖香的茶叶，沸水倾注下去的刹那，氤氲的热气便在书桌前搭起了一小片隔开喧嚣的屋檐。

# 二 · 书页里的慢拍子
白天总被各种进度条和提醒声切得很碎，好像慢半拍就会错过什么。可真正坐下来听雨、捧一杯热茶翻开书时，才会发觉世界并没有因为自己停下二十分钟而变得慌乱。

茶汤第一泡清亮，第二泡醇厚，第三泡渐渐柔和下来。读一本书也是如此：不必急着把每一句都背下来，允许某些段落轻轻滑过心尖，留下一点清欢与余韵就已足够。

# 三 · 檐下余温
雨声在十点以后变得疏落，茶壶里的水也渐渐凉成温润的体温。合上书，夹进一张随手折好的牛皮纸书签，桌面留白处映着台灯暖黄的光晕。

这一夜什么宏大的决定都没有做，却奇异地觉得心里稳当了许多。原来照顾好自己，常常只是烧一壶热水、读几页安静的字。
"#;

const QUIET_HARBOR_LETTERS_TEXT: &str = r#"# 第一封 · 写给觉得自己落后的夜晚
亲爱的朋友：
如果你今天又因为清单上没划掉的几项而感到愧疚，请先在这行字后面停一停，把肩膀轻轻放下来。

生活不是百米冲刺，更像沿着一条曲折的林间溪流散步。有人在前面的弯道看风景，有人蹲在石阶旁数青苔，并没有哪一步算是白走。今晚不谈追赶，只谈歇脚。

# 第二封 · 写给找不到答案的午后
迷茫的时候，人总想立刻翻到最后一页看结局。可树苗在土里扎根的那几个月，地面上也是看不见任何变化的。

如果眼前一时起雾，不必硬要看清三公里以外的路标。只把目光收回到脚下这一步，读完手边这一小节，吃一顿热乎的晚饭，把房间收拾出一个干净的角落——微小的秩序会悄悄托住你的心。

# 第三封 · 写给正在慢慢恢复力气的你
复原从来不是一条笔直向上的折线。有时明明已经轻松了三天，第四天忽然又涌起一阵无来由的疲倦，那并不代表你退回了原地，只是身体在提醒你：刚才走得稍微有点快了。

允许自己有情绪的潮汐，也允许自己在需要的时候退回安静的港湾。等你准备好了，风会自然而然推着船帆重新启程。
"#;

const LANTERN_BILINGUAL_TEXT: &str = r#"# Chapter One · The Small Lamp (一盏小灯)
A small lamp on the desk does not compete with the noon sun. It asks only to keep one quiet circle warm when the evening settles over the rooftops.
书桌上的小台灯从不与正午的烈日争辉。当夜色落满屋檐，它只安静地守住眼前这一小圈温柔的光亮。

To read slowly is to refuse the rumor that every hour must be hurried. Between two sentences, there is room enough to breathe.
慢读，是温和地拒绝“每一小时都必须奔忙”的传言。在两句文字的停顿之间，本就留着足够呼吸的空隙。

# Chapter Two · Gentle Companionship (轻声相伴)
Good books wait without impatience. You may close the cover for three weeks, wander through noisy seasons, and return on a rainy Tuesday to find the same page still holding your place.
好书从不催促。你可以合上书页三周，穿过喧闹的季节，在一个下雨的周二夜晚重新回来，那一页依然安静地替你留着位置。

Carry one clear sentence into sleep, the way a traveler carries a warm pebble in a coat pocket. Tomorrow will unfold in its own time.
带一句清澈的句子入睡，就像旅人在大衣口袋里揣着一枚温热的鹅卵石。明天会按照它自己的节奏缓缓展开。
"#;

pub const CATALOG_BOOKS: &[CatalogBook] = &[
    CatalogBook {
        id: "forest-slow",
        title: "在林间，慢慢来",
        author: "微光阅读 · 原创散文",
        category: "原创散文",
        era: "原创常驻",
        minutes: 8,
        moods: &["疲惫", "睡前", "独处"],
        tagline: "写给忙碌之后想喘一口气的人：把灯调暖，允许自己什么都不赶。",
        accent_hex: "#2573B8",
        content: SAMPLE_FOREST,
    },
    CatalogBook {
        id: "starlight-post",
        title: "星灯与慢邮局",
        author: "微光阅读 · 睡前故事",
        category: "睡前故事",
        era: "原创治愈",
        minutes: 7,
        moods: &["睡前", "疲惫", "希望"],
        tagline: "山腰上只在傍晚亮灯的慢邮局，收容没写完的心事与不必着急的明天。",
        accent_hex: "#4B6A9B",
        content: STARLIGHT_POST_TEXT,
    },
    CatalogBook {
        id: "rainy-tea-notes",
        title: "雨夜煮茶手记",
        author: "微光阅读 · 生活随笔",
        category: "原创散文",
        era: "原创治愈",
        minutes: 6,
        moods: &["疲惫", "独处", "睡前"],
        tagline: "烧一壶水、泡一盏焙火乌龙，在细雨声里把紧绷的节奏松开半拍。",
        accent_hex: "#6B7C59",
        content: RAINY_TEA_NOTES_TEXT,
    },
    CatalogBook {
        id: "quiet-harbor-letters",
        title: "写给慢下来的十封信",
        author: "微光阅读 · 书信札记",
        category: "原创散文",
        era: "原创治愈",
        minutes: 9,
        moods: &["迷茫", "希望", "独处"],
        tagline: "当清单太长、步伐太急时，打开一封不讲大道理的安静长信。",
        accent_hex: "#9E6A44",
        content: QUIET_HARBOR_LETTERS_TEXT,
    },
    CatalogBook {
        id: "taohuayuan",
        title: "归园田居与桃花源",
        author: "陶渊明",
        category: "古典诗文",
        era: "东晋 · 公版经典",
        minutes: 10,
        moods: &["疲惫", "迷茫", "独处"],
        tagline: "收录《桃花源记》《归去来兮辞》《饮酒》，在喧嚣之外留一扇南窗。",
        accent_hex: "#3D7A52",
        content: TAOYUANMING_TEXT,
    },
    CatalogBook {
        id: "chibi-sushi",
        title: "赤壁夜游集",
        author: "苏轼",
        category: "古典诗文",
        era: "北宋 · 公版经典",
        minutes: 12,
        moods: &["迷茫", "独处", "睡前"],
        tagline: "清风明月与竹杖芒鞋：失意时读东坡，把风雨读成晴朗。",
        accent_hex: "#B84B42",
        content: SUSHI_TEXT,
    },
    CatalogBook {
        id: "wangwei-autumn",
        title: "辋川山居诗笺",
        author: "王维",
        category: "古典诗文",
        era: "盛唐 · 公版经典",
        minutes: 6,
        moods: &["睡前", "独处", "疲惫"],
        tagline: "空山新雨、明月松间，七首适合睡前慢读的安静山水诗。",
        accent_hex: "#63549E",
        content: WANGWEI_TEXT,
    },
    CatalogBook {
        id: "walden-solitude",
        title: "瓦尔登湖 · 独处选篇",
        author: "Henry David Thoreau",
        category: "双语原典",
        era: "1854 · 英文公版",
        minutes: 14,
        moods: &["独处", "迷茫"],
        tagline: "梭罗关于清醒生活与独处的原典选章，适合静夜慢慢咀嚼。",
        accent_hex: "#536A49",
        content: WALDEN_TEXT,
    },
    CatalogBook {
        id: "stray-birds",
        title: "飞鸟集 · 静夜选札",
        author: "Rabindranath Tagore",
        category: "双语原典",
        era: "1916 · 双语公版",
        minutes: 7,
        moods: &["睡前", "疲惫", "希望"],
        tagline: "中英对照的短诗微光，一页只需两分钟，温柔接住睡前思绪。",
        accent_hex: "#AE572A",
        content: STRAY_BIRDS_TEXT,
    },
    CatalogBook {
        id: "lantern-bilingual-notes",
        title: "微光双语慢读札记 · Quiet Hours",
        author: "微光阅读 · 双语随笔",
        category: "双语原典",
        era: "原创双语",
        minutes: 8,
        moods: &["独处", "睡前", "希望"],
        tagline: "中英对照的夜晚短札，适合一边听雨一边轻声慢读。",
        accent_hex: "#3E6D8C",
        content: LANTERN_BILINGUAL_TEXT,
    },
];

pub fn all_books() -> &'static [CatalogBook] {
    CATALOG_BOOKS
}

pub fn find(id: &str) -> Option<&'static CatalogBook> {
    CATALOG_BOOKS.iter().find(|b| b.id == id)
}

pub fn filter_by_category(category: &str) -> Vec<&'static CatalogBook> {
    let trimmed = category.trim();
    if trimmed.is_empty() || trimmed == "全部" {
        return CATALOG_BOOKS.iter().collect();
    }
    if trimmed == "10分钟内" {
        return CATALOG_BOOKS.iter().filter(|b| b.minutes <= 10).collect();
    }
    CATALOG_BOOKS
        .iter()
        .filter(|b| b.category == trimmed)
        .collect()
}

pub fn recommend_for_mood(mood: &str) -> Vec<&'static CatalogBook> {
    let trimmed = mood.trim();
    if trimmed.is_empty() || trimmed == "全部" {
        return CATALOG_BOOKS.iter().collect();
    }
    let matched: Vec<&'static CatalogBook> = CATALOG_BOOKS
        .iter()
        .filter(|book| book.moods.contains(&trimmed))
        .collect();
    if matched.is_empty() {
        CATALOG_BOOKS.iter().take(3).collect()
    } else {
        matched
    }
}

impl Store {
    /// Installs a curated public-domain or original book from the built-in 微光公版馆.
    pub fn install_catalog_book(&mut self, catalog_id: &str) -> Result<String> {
        let entry = find(catalog_id).context("微光公版馆中未找到这本书")?;
        let bytes = text_epub(entry.content, entry.title, entry.author)?;
        let book_id = self.import_epub_bytes(bytes)?;
        if let Some(book) = self.state.books.iter_mut().find(|b| b.id == book_id)
            && book.tags.is_empty()
        {
            book.tags.push("微光公版馆".into());
            let _ = self.save();
        }
        Ok(book_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_mood_recommendations_and_install_produce_valid_epubs() {
        assert!(all_books().len() >= 10);
        let bedtime = recommend_for_mood("睡前");
        assert!(bedtime.len() >= 5);
        assert!(
            filter_by_category("睡前故事")
                .iter()
                .any(|b| b.id == "starlight-post")
        );
        assert!(
            filter_by_category("10分钟内")
                .iter()
                .all(|b| b.minutes <= 10)
        );
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(dir.path().to_path_buf()).unwrap();
        let id = store.install_catalog_book("chibi-sushi").unwrap();
        let book = store.state.books.iter().find(|b| b.id == id).unwrap();
        assert_eq!(book.title, "赤壁夜游集");
        assert_eq!(book.author, "苏轼");
        assert!(
            store
                .root
                .join("books")
                .join(&id)
                .join("book.epub")
                .exists()
        );
        // Deduplicated on second install
        let id2 = store.install_catalog_book("chibi-sushi").unwrap();
        assert_eq!(id, id2);
        assert_eq!(store.state.books.len(), 1);

        // Newly added original book installs cleanly too
        let story_id = store.install_catalog_book("starlight-post").unwrap();
        let story = store.state.books.iter().find(|b| b.id == story_id).unwrap();
        assert_eq!(story.title, "星灯与慢邮局");
        assert!(story.tags.iter().any(|t| t == "微光公版馆"));
        assert!(
            store
                .root
                .join("books")
                .join(&story_id)
                .join("book.epub")
                .exists()
        );
    }
}
