//! QUEST 상태·장비·도구 고정문과 실제 glyph 소비 경계.
//!
//! 원본 NUL slot과 모든 주소 load를 결속한다. popup은 네 caller가
//! 세 고정문만 공급하는 공용 loop이므로 해당 loop를 한 번만 연결한다.
use super::*;

pub const QUEST_MENU_LITERAL_COUNT: usize = 108;

pub const QUEST_MENU_LITERAL_SPECS: &[(&str, u32, u32, &str)] = &[
    ("equipment-none", 0x8001_0FB0, 0x8001_0FC0, "装備しないよ"),
    ("speed", 0x8001_0FC0, 0x8001_0FCC, "すばやさ"),
    ("element", 0x8001_0FCC, 0x8001_0FD8, "ぞくせい"),
    (
        "equipment-remove",
        0x8001_0FD8,
        0x8001_0FE8,
        "装備をはずすよ",
    ),
    (
        "equipment-bag-full",
        0x8001_0FE8,
        0x8001_1008,
        "持ち物がいっぱいではずせないよ",
    ),
    ("finish", 0x8001_1008, 0x8001_1014, "おわるよ"),
    ("special-skill", 0x8001_1014, 0x8001_1024, "　　　　　特技"),
    (
        "equipment-empty",
        0x8001_1024,
        0x8001_1034,
        "装備してないよ",
    ),
    (
        "skill-equip-finish",
        0x8001_1034,
        0x8001_104C,
        "特技の装備を　おわるよ",
    ),
    ("key-items", 0x8001_104C, 0x8001_105C, "だいじなもの"),
    ("break-wand", 0x8001_105C, 0x8001_106C, "バラバラの杖"),
    ("strong-wand", 0x8001_106C, 0x8001_107C, "オラオラの杖"),
    ("normal-wand", 0x8001_107C, 0x8001_1088, "ふつうの杖"),
    ("boots", 0x8001_1088, 0x8001_109C, "コンニャクブーツ"),
    ("gloves", 0x8001_109C, 0x8001_10B0, "かいりきグローブ"),
    ("battle-badge", 0x8001_10B0, 0x8001_10C0, "たいせんバッジ"),
    ("hot-tea", 0x8001_10C0, 0x8001_10D0, "アツーイお茶"),
    ("earplugs", 0x8001_10D0, 0x8001_10DC, "みみせん"),
    ("player-name", 0x8001_10DC, 0x8001_10F0, "　アルル・ナジャ"),
    ("experience-next", 0x8001_10F0, 0x8001_10FC, "　　あと"),
    ("play-time", 0x8001_10FC, 0x8001_110C, "じかん　　　　"),
    ("equipment-slot-one", 0x8001_110C, 0x8001_1118, "装備　１"),
    ("equipment-slot-two", 0x8001_1118, 0x8001_1124, "装備　２"),
    ("equipment-slot-three", 0x8001_1124, 0x8001_1130, "装備　３"),
    (
        "return-box",
        0x8001_1130,
        0x8001_1150,
        "ぷよＢＯＸメニューに　もどるよ",
    ),
    (
        "return-box-confirm",
        0x8001_1150,
        0x8001_116C,
        "ほんとに　もどっていい？",
    ),
    ("status-menu", 0x8001_116C, 0x8001_1178, "ステータス"),
    ("discard-suffix", 0x8001_1178, 0x8001_1188, "を　すてるよ"),
    (
        "discard-confirm",
        0x8001_1188,
        0x8001_119C,
        "ほんとに　いいの？",
    ),
    (
        "sell-confirm",
        0x8001_119C,
        0x8001_11B0,
        "これを売っていい？",
    ),
    (
        "buy-confirm",
        0x8001_11B0,
        0x8001_11C4,
        "これを買っていい？",
    ),
    ("item-element", 0x8001_11C4, 0x8001_11D4, "ぞくせい　　"),
    (
        "item-consumed",
        0x8001_11D4,
        0x8001_11F0,
        "　１　ど使うと　なくなるよ",
    ),
    (
        "item-uses-suffix",
        0x8001_11F0,
        0x8001_1200,
        "　回　使えるよ",
    ),
    ("bag-full", 0x8001_1200, 0x8001_1218, "持ち物がいっぱいだよ"),
    ("bag-empty", 0x8001_1218, 0x8001_122C, "何も持っていません"),
    ("item-use-subject", 0x8001_190C, 0x8001_191C, "　　アルルは"),
    ("item-used", 0x8001_191C, 0x8001_192C, "　　使った！！"),
    (
        "item-no-effect",
        0x8001_192C,
        0x8001_1950,
        "　　しかし、何もおこらなかった！！",
    ),
    (
        "attack-increased-subject",
        0x8001_1950,
        0x8001_1968,
        "　　アルルの攻撃力が　",
    ),
    (
        "stat-increased-suffix",
        0x8001_1968,
        0x8001_1978,
        "　上がった！！",
    ),
    (
        "defense-increased-subject",
        0x8001_1978,
        0x8001_1990,
        "　　アルルの防御力が　",
    ),
    (
        "speed-increased-subject",
        0x8001_1990,
        0x8001_19AC,
        "　　アルルのすばやさが　",
    ),
    (
        "invisible-subject",
        0x8001_19AC,
        0x8001_19C4,
        "　　アルルのすがたが",
    ),
    (
        "invisible-effect",
        0x8001_19C4,
        0x8001_19E0,
        "　　敵からみえなくなった！",
    ),
    ("recover", 0x800B_E09C, 0x800B_E0A4, "回復"),
    ("save-menu", 0x800B_E0A4, 0x800B_E0AC, "セーブ"),
    ("attack", 0x800B_E0AC, 0x800B_E0B4, "攻撃力"),
    ("defense", 0x800B_E0B4, 0x800B_E0BC, "防御力"),
    ("equipment-one", 0x800B_E0BC, 0x800B_E0C4, "装備１"),
    ("equipment-two", 0x800B_E0C4, 0x800B_E0CC, "装備２"),
    ("equipment-three", 0x800B_E0CC, 0x800B_E0D4, "装備３"),
    ("experience", 0x800B_E0DC, 0x800B_E0E4, "経験値"),
    ("money", 0x800B_E0E4, 0x800B_E0EC, "お金"),
    ("medals", 0x800B_E0F0, 0x800B_E0F8, "メダル"),
    ("equipment-menu", 0x800B_E108, 0x800B_E110, "装備"),
    ("items-menu", 0x800B_E110, 0x800B_E118, "道具"),
    ("load-menu", 0x800B_E118, 0x800B_E120, "ロード"),
    ("back-menu", 0x800B_E120, 0x800B_E128, "もどる"),
    ("use-menu", 0x800B_E128, 0x800B_E130, "つかう"),
    ("discard-menu", 0x800B_E130, 0x800B_E138, "すてる"),
    ("item-uses-prefix", 0x800B_E138, 0x800B_E140, "あと　"),
    ("buy-menu", 0x800B_E140, 0x800B_E148, "買う"),
    ("sell-menu", 0x800B_E148, 0x800B_E150, "売る"),
    ("buy-selected", 0x800B_E150, 0x800B_E158, "　買う"),
    ("sell-selected", 0x800B_E158, 0x800B_E160, "　売る"),
    ("flight-planbar", 0x8001_0BB8, 0x8001_0BC8, "プランバーの村"),
    (
        "flight-cherrybell",
        0x8001_0BC8,
        0x8001_0BD8,
        "チェリベルの町",
    ),
    (
        "flight-sevenlease",
        0x8001_0BD8,
        0x8001_0BEC,
        "セブンリースの村",
    ),
    ("flight-pakista", 0x8001_0BEC, 0x8001_0BFC, "パキスタの店"),
    ("flight-parara", 0x8001_0BFC, 0x8001_0C08, "パララの店"),
    (
        "flight-sasoriman",
        0x8001_0C08,
        0x8001_0C18,
        "さそりまんの店",
    ),
    (
        "card-checking",
        0x8001_0C18,
        0x8001_0C40,
        "　　メモリーカードをチェックしてるよ",
    ),
    (
        "card-do-not-remove",
        0x8001_0C40,
        0x8001_0C64,
        "　　メモリーカードやコントローラを",
    ),
    (
        "card-do-not-remove-end",
        0x8001_0C64,
        0x8001_0C80,
        "　　ぬきさし　しないでね",
    ),
    (
        "card-load-select",
        0x8001_0C80,
        0x8001_0C98,
        "　　どれをロードする？",
    ),
    ("card-level", 0x8001_0C98, 0x8001_0CA4, "レベル　"),
    (
        "card-empty-slot",
        0x8001_0CA4,
        0x8001_0CB4,
        "データがないよ",
    ),
    (
        "card-empty-notice",
        0x8001_0CB4,
        0x8001_0CC8,
        "　　データがないよ",
    ),
    (
        "card-loading",
        0x8001_0CC8,
        0x8001_0CDC,
        "　　ロードしてるよ",
    ),
    (
        "card-access-error",
        0x8001_0CDC,
        0x8001_0CFC,
        "　　メモリカードへのアクセスに",
    ),
    (
        "card-access-error-end",
        0x8001_0CFC,
        0x8001_0D10,
        "　　しっぱいしたよ",
    ),
    (
        "card-load-failed",
        0x8001_0D10,
        0x8001_0D30,
        "　　ロードに　しっぱいしたよ",
    ),
    (
        "card-not-inserted",
        0x8001_0D30,
        0x8001_0D58,
        "　　メモリーカードが　スロット　１　に",
    ),
    (
        "card-not-inserted-end",
        0x8001_0D58,
        0x8001_0D6C,
        "　　ささってないよ",
    ),
    (
        "card-changed",
        0x8001_0D6C,
        0x8001_0D94,
        "　　メモリーカードが　さっきとちがうよ",
    ),
    (
        "card-unformatted",
        0x8001_0D94,
        0x8001_0DB4,
        "　　フォーマットされてないよ",
    ),
    (
        "card-load-retry",
        0x8001_0DB4,
        0x8001_0DD0,
        "　　まだロードをつづける？",
    ),
    (
        "card-no-game-data",
        0x8001_0DD0,
        0x8001_0DF4,
        "　　ぷよぷよＢＯＸのデータがないよ",
    ),
    (
        "card-save-select",
        0x8001_0DF4,
        0x8001_0E0C,
        "　　どこにセーブする？",
    ),
    (
        "card-overwrite-intro",
        0x8001_0E0C,
        0x8001_0E1C,
        "　　そうすると",
    ),
    (
        "card-overwrite-level",
        0x8001_0E1C,
        0x8001_0E2C,
        "　　レベル　",
    ),
    (
        "card-overwrite-data",
        0x8001_0E2C,
        0x8001_0E4C,
        "　　のデータが消えちゃうけど",
    ),
    (
        "card-overwrite-confirm",
        0x8001_0E4C,
        0x8001_0E5C,
        "　　いいかな？",
    ),
    (
        "card-saving",
        0x8001_0E5C,
        0x8001_0E70,
        "　　セーブしてるよ",
    ),
    ("card-saved", 0x8001_0E70, 0x8001_0E84, "　　セーブしたよ"),
    (
        "card-save-failed",
        0x8001_0E84,
        0x8001_0EA4,
        "　　セーブに　しっぱいしたよ",
    ),
    (
        "card-space-insufficient",
        0x8001_0EA4,
        0x8001_0EC4,
        "　　あきブロックが　たりないよ",
    ),
    (
        "card-space-required",
        0x8001_0EC4,
        0x8001_0EE8,
        "　　セーブには　１　ブロックいるよ",
    ),
    (
        "card-save-retry",
        0x8001_0EE8,
        0x8001_0F04,
        "　　まだセーブをつづける？",
    ),
    (
        "card-format-confirm",
        0x8001_0F04,
        0x8001_0F1C,
        "　　フォーマットする？",
    ),
    (
        "card-formatting",
        0x8001_0F1C,
        0x8001_0F38,
        "　　フォーマットしてるよ",
    ),
    (
        "card-formatted",
        0x8001_0F38,
        0x8001_0F50,
        "　　フォーマットしたよ",
    ),
    (
        "card-format-failed",
        0x8001_0F50,
        0x8001_0F74,
        "　　フォーマットにしっぱいしたよ",
    ),
    (
        "card-create-data",
        0x8001_0F74,
        0x8001_0F94,
        "　　ぷよぷよＢＯＸのデータを",
    ),
    (
        "card-create-confirm",
        0x8001_0F94,
        0x8001_0FB0,
        "　　あたらしく　つくるよ？",
    ),
    ("card-north-river", 0x800B_E088, 0x800B_E090, "北の川"),
    ("world-north-river", 0x800B_E23C, 0x800B_E244, "北の川"),
];

pub(super) const REFERENCES: &[(u32, &[(u32, u32)])] = &[
    (0x80010fb0, &[(0x80031288, 0x8003128c)]),
    (
        0x80010fc0,
        &[(0x800319cc, 0x800319d0), (0x8003472c, 0x80034730)],
    ),
    (
        0x80010fcc,
        &[(0x80031afc, 0x80031b00), (0x80034b48, 0x80034b4c)],
    ),
    (0x80010fd8, &[(0x80031d2c, 0x80031d30)]),
    (
        0x80010fe8,
        &[(0x80031f70, 0x80031f74), (0x80032cb0, 0x80032cb4)],
    ),
    (0x80011008, &[(0x800324e0, 0x800324e4)]),
    (0x80011014, &[(0x80032724, 0x80032728)]),
    (
        0x80011024,
        &[
            (0x8003289c, 0x800328a0),
            (0x800330ec, 0x800330f0),
            (0x80033280, 0x80033284),
            (0x80033410, 0x80033414),
            (0x80035438, 0x8003543c),
            (0x8003559c, 0x800355a0),
            (0x800356fc, 0x80035700),
            (0x80036d98, 0x80036d9c),
            (0x80036f2c, 0x80036f30),
            (0x800370bc, 0x800370c0),
        ],
    ),
    (0x80011034, &[(0x800329dc, 0x800329e0)]),
    (0x8001104c, &[(0x80033920, 0x80033924)]),
    (0x8001105c, &[(0x800339e0, 0x800339e4)]),
    (0x8001106c, &[(0x80033a30, 0x80033a34)]),
    (0x8001107c, &[(0x80033a78, 0x80033a7c)]),
    (0x80011088, &[(0x80033b00, 0x80033b04)]),
    (0x8001109c, &[(0x80033b90, 0x80033b94)]),
    (0x800110b0, &[(0x80033c18, 0x80033c1c)]),
    (0x800110c0, &[(0x80033ca8, 0x80033cac)]),
    (0x800110d0, &[(0x80033d30, 0x80033d34)]),
    (0x800110dc, &[(0x800342cc, 0x800342d0)]),
    (0x800110f0, &[(0x8003496c, 0x80034970)]),
    (
        0x800110fc,
        &[
            (0x800352b8, 0x800352bc),
            (0x800358a4, 0x800358a8),
            (0x80035ac0, 0x80035ac4),
        ],
    ),
    (0x8001110c, &[(0x80035320, 0x80035324)]),
    (0x80011118, &[(0x80035480, 0x80035484)]),
    (0x80011124, &[(0x800355e4, 0x800355e8)]),
    (0x80011130, &[(0x80035e18, 0x80035e1c)]),
    (0x80011150, &[(0x80035e64, 0x80035e68)]),
    (0x8001116c, &[(0x80036588, 0x8003658c)]),
    (0x80011178, &[(0x80037c6c, 0x80037c70)]),
    (0x80011188, &[(0x80037cb8, 0x80037cbc)]),
    (0x8001119c, &[(0x800399c8, 0x800399cc)]),
    (0x800111b0, &[(0x80039a10, 0x80039a14)]),
    (0x800111c4, &[(0x80039ed0, 0x80039ed4)]),
    (
        0x800111d4,
        &[(0x8003a120, 0x8003a124), (0x8003a254, 0x8003a258)],
    ),
    (
        0x800111f0,
        &[(0x8003a1e0, 0x8003a1e4), (0x8003a328, 0x8003a32c)],
    ),
    (0x80011200, &[(0x8003ae04, 0x8003ae08)]),
    (0x80011218, &[(0x8003b6ec, 0x8003b6f0)]),
    (0x8001190c, &[(0x80053378, 0x80053380)]),
    (0x8001191c, &[(0x800533ac, 0x800533b4)]),
    (0x8001192c, &[(0x800534e8, 0x800534f0)]),
    (0x80011950, &[(0x80053680, 0x80053688)]),
    (
        0x80011968,
        &[
            (0x800536b8, 0x800536c0),
            (0x80053740, 0x80053748),
            (0x800537c8, 0x800537d0),
        ],
    ),
    (0x80011978, &[(0x80053708, 0x80053710)]),
    (0x80011990, &[(0x80053790, 0x80053798)]),
    (0x800119ac, &[(0x80053940, 0x80053948)]),
    (0x800119c4, &[(0x80053958, 0x80053960)]),
    (0x800be09c, &[(0x8003058c, 0x80030590)]),
    (0x800be0a4, &[(0x80030624, 0x80030628)]),
    (
        0x800be0ac,
        &[(0x8003171c, 0x80031720), (0x80034474, 0x80034478)],
    ),
    (
        0x800be0b4,
        &[(0x80031874, 0x80031878), (0x800345d0, 0x800345d4)],
    ),
    (
        0x800be0bc,
        &[(0x80032fa4, 0x80032fa8), (0x80036c50, 0x80036c54)],
    ),
    (
        0x800be0c4,
        &[(0x80033138, 0x8003313c), (0x80036de4, 0x80036de8)],
    ),
    (
        0x800be0cc,
        &[(0x800332cc, 0x800332d0), (0x80036f78, 0x80036f7c)],
    ),
    (0x800be0dc, &[(0x800348a8, 0x800348ac)]),
    (0x800be0e4, &[(0x80034a48, 0x80034a4c)]),
    (0x800be0f0, &[(0x80034d48, 0x80034d4c)]),
    (0x800be108, &[(0x80036620, 0x80036624)]),
    (0x800be110, &[(0x800366f8, 0x800366fc)]),
    (0x800be118, &[(0x80036794, 0x80036798)]),
    (0x800be120, &[(0x80036850, 0x80036854)]),
    (0x800be128, &[(0x8003826c, 0x80038270)]),
    (0x800be130, &[(0x80038304, 0x80038308)]),
    (
        0x800be138,
        &[(0x8003a180, 0x8003a184), (0x8003a2c8, 0x8003a2cc)],
    ),
    (0x800be140, &[(0x8003b128, 0x8003b12c)]),
    (0x800be148, &[(0x8003b1c0, 0x8003b1c4)]),
    (0x800be150, &[(0x8003b3e0, 0x8003b3e4)]),
    (0x800be158, &[(0x8003b52c, 0x8003b530)]),
    (
        0x8001_0BB8,
        &[(0x8002_D2B4, 0x8002_D2B8), (0x8003_0B1C, 0x8003_0B20)],
    ),
    (
        0x8001_0BC8,
        &[(0x8002_D2F8, 0x8002_D2FC), (0x8003_0B64, 0x8003_0B68)],
    ),
    (
        0x8001_0BD8,
        &[(0x8002_D33C, 0x8002_D340), (0x8003_0C3C, 0x8003_0C40)],
    ),
    (
        0x8001_0BEC,
        &[(0x8002_D380, 0x8002_D384), (0x8003_0BAC, 0x8003_0BB0)],
    ),
    (
        0x8001_0BFC,
        &[(0x8002_D3C4, 0x8002_D3C8), (0x8003_0BF4, 0x8003_0BF8)],
    ),
    (
        0x8001_0C08,
        &[(0x8002_D408, 0x8002_D40C), (0x8003_0C84, 0x8003_0C88)],
    ),
    (
        0x8001_0C18,
        &[(0x8002_D7B8, 0x8002_D7BC), (0x8002_EA58, 0x8002_EA5C)],
    ),
    (
        0x8001_0C40,
        &[
            (0x8002_D804, 0x8002_D808),
            (0x8002_E110, 0x8002_E114),
            (0x8002_EAA4, 0x8002_EAA8),
            (0x8002_F5C4, 0x8002_F5C8),
            (0x8002_FF30, 0x8002_FF34),
        ],
    ),
    (
        0x8001_0C64,
        &[
            (0x8002_D850, 0x8002_D854),
            (0x8002_E15C, 0x8002_E160),
            (0x8002_EAF0, 0x8002_EAF4),
            (0x8002_F610, 0x8002_F614),
            (0x8002_FF7C, 0x8002_FF80),
        ],
    ),
    (0x8001_0C80, &[(0x8002_D9D0, 0x8002_D9D4)]),
    (
        0x8001_0C98,
        &[(0x8002_DB58, 0x8002_DB5C), (0x8002_EE38, 0x8002_EE3C)],
    ),
    (
        0x8001_0CA4,
        &[(0x8002_DCEC, 0x8002_DCF0), (0x8002_EFCC, 0x8002_EFD0)],
    ),
    (0x8001_0CB4, &[(0x8002_E02C, 0x8002_E030)]),
    (0x8001_0CC8, &[(0x8002_E0C4, 0x8002_E0C8)]),
    (
        0x8001_0CDC,
        &[(0x8002_E3D0, 0x8002_E3D4), (0x8002_F978, 0x8002_F97C)],
    ),
    (
        0x8001_0CFC,
        &[(0x8002_E41C, 0x8002_E420), (0x8002_F9C4, 0x8002_F9C8)],
    ),
    (0x8001_0D10, &[(0x8002_E480, 0x8002_E484)]),
    (
        0x8001_0D30,
        &[(0x8002_E4E4, 0x8002_E4E8), (0x8002_FB5C, 0x8002_FB60)],
    ),
    (
        0x8001_0D58,
        &[(0x8002_E530, 0x8002_E534), (0x8002_FBA8, 0x8002_FBAC)],
    ),
    (
        0x8001_0D6C,
        &[(0x8002_E594, 0x8002_E598), (0x8002_FC0C, 0x8002_FC10)],
    ),
    (
        0x8001_0D94,
        &[(0x8002_E5F8, 0x8002_E5FC), (0x8002_FDF8, 0x8002_FDFC)],
    ),
    (0x8001_0DB4, &[(0x8002_E67C, 0x8002_E680)]),
    (
        0x8001_0DD0,
        &[(0x8002_E768, 0x8002_E76C), (0x8003_0144, 0x8003_0148)],
    ),
    (0x8001_0DF4, &[(0x8002_ECB0, 0x8002_ECB4)]),
    (0x8001_0E0C, &[(0x8002_F2EC, 0x8002_F2F0)]),
    (0x8001_0E1C, &[(0x8002_F338, 0x8002_F33C)]),
    (0x8001_0E2C, &[(0x8002_F468, 0x8002_F46C)]),
    (0x8001_0E4C, &[(0x8002_F4B4, 0x8002_F4B8)]),
    (0x8001_0E5C, &[(0x8002_F578, 0x8002_F57C)]),
    (0x8001_0E70, &[(0x8002_F7A8, 0x8002_F7AC)]),
    (0x8001_0E84, &[(0x8002_FA48, 0x8002_FA4C)]),
    (0x8001_0EA4, &[(0x8002_FAAC, 0x8002_FAB0)]),
    (0x8001_0EC4, &[(0x8002_FAF8, 0x8002_FAFC)]),
    (0x8001_0EE8, &[(0x8002_FC90, 0x8002_FC94)]),
    (0x8001_0F04, &[(0x8002_FE44, 0x8002_FE48)]),
    (0x8001_0F1C, &[(0x8002_FEE4, 0x8002_FEE8)]),
    (0x8001_0F38, &[(0x8003_0038, 0x8003_003C)]),
    (0x8001_0F50, &[(0x8003_00B8, 0x8003_00BC)]),
    (0x8001_0F74, &[(0x8003_01C0, 0x8003_01C4)]),
    (0x8001_0F94, &[(0x8003_020C, 0x8003_0210)]),
    (0x800B_E088, &[(0x8002_D44C, 0x8002_D450)]),
    (0x800B_E23C, &[(0x8006_6378, 0x8006_6380)]),
];

#[derive(Debug, Clone, Copy)]
pub struct QuestMenuGlyphCall {
    pub runtime: u32,
    pub whole_string: bool,
    pub delay: K,
    pub id: &'static str,
}

pub const QUEST_MENU_GLYPH_CALLS: &[QuestMenuGlyphCall] = &[
    QuestMenuGlyphCall {
        runtime: 0x8002_D0C8,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg::S0,
            rs: Reg::S0,
            imm: 2,
        },
        id: "quest-menu-glyph-site-8002d0c8",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_05B8,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg::S0,
            rs: Reg::S0,
            imm: 2,
        },
        id: "quest-menu-glyph-site-800305b8",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_064C,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg::S0,
            rs: Reg::S0,
            imm: 2,
        },
        id: "quest-menu-glyph-site-8003064c",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_12B4,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg::S0,
            rs: Reg::S0,
            imm: 2,
        },
        id: "quest-menu-glyph-site-800312b4",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_174C,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg::S0,
            rs: Reg::S0,
            imm: 2,
        },
        id: "quest-menu-glyph-site-8003174c",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_18A4,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg::S0,
            rs: Reg::S0,
            imm: 2,
        },
        id: "quest-menu-glyph-site-800318a4",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_19FC,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg::S0,
            rs: Reg::S0,
            imm: 2,
        },
        id: "quest-menu-glyph-site-800319fc",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_1B2C,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg::S0,
            rs: Reg::S0,
            imm: 2,
        },
        id: "quest-menu-glyph-site-80031b2c",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_1D5C,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg::S0,
            rs: Reg::S0,
            imm: 2,
        },
        id: "quest-menu-glyph-site-80031d5c",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_250C,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg::S0,
            rs: Reg::S0,
            imm: 2,
        },
        id: "quest-menu-glyph-site-8003250c",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_2758,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg::S0,
            rs: Reg::S0,
            imm: 2,
        },
        id: "quest-menu-glyph-site-80032758",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_28D0,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg::S0,
            rs: Reg::S0,
            imm: 2,
        },
        id: "quest-menu-glyph-site-800328d0",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_2A08,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg::S0,
            rs: Reg::S0,
            imm: 2,
        },
        id: "quest-menu-glyph-site-80032a08",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_2FD0,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg::S0,
            rs: Reg::S0,
            imm: 2,
        },
        id: "quest-menu-glyph-site-80032fd0",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_3118,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg::S0,
            rs: Reg::S0,
            imm: 2,
        },
        id: "quest-menu-glyph-site-80033118",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_3164,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg::S0,
            rs: Reg::S0,
            imm: 2,
        },
        id: "quest-menu-glyph-site-80033164",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_32AC,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg::S0,
            rs: Reg::S0,
            imm: 2,
        },
        id: "quest-menu-glyph-site-800332ac",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_32F8,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg::S0,
            rs: Reg::S0,
            imm: 2,
        },
        id: "quest-menu-glyph-site-800332f8",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_343C,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg::S0,
            rs: Reg::S0,
            imm: 2,
        },
        id: "quest-menu-glyph-site-8003343c",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_3980,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg::S0,
            rs: Reg::S0,
            imm: 2,
        },
        id: "quest-menu-glyph-site-80033980",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_3A0C,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg::S0,
            rs: Reg::S0,
            imm: 2,
        },
        id: "quest-menu-glyph-site-80033a0c",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_3A5C,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg::S0,
            rs: Reg::S0,
            imm: 2,
        },
        id: "quest-menu-glyph-site-80033a5c",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_3AA4,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg::S0,
            rs: Reg::S0,
            imm: 2,
        },
        id: "quest-menu-glyph-site-80033aa4",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_3B2C,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg::S0,
            rs: Reg::S0,
            imm: 2,
        },
        id: "quest-menu-glyph-site-80033b2c",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_3BBC,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg::S0,
            rs: Reg::S0,
            imm: 2,
        },
        id: "quest-menu-glyph-site-80033bbc",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_3C44,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg::S0,
            rs: Reg::S0,
            imm: 2,
        },
        id: "quest-menu-glyph-site-80033c44",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_3CD4,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg::S0,
            rs: Reg::S0,
            imm: 2,
        },
        id: "quest-menu-glyph-site-80033cd4",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_3D5C,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg::S0,
            rs: Reg::S0,
            imm: 2,
        },
        id: "quest-menu-glyph-site-80033d5c",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_42F8,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg::S1,
            rs: Reg::S1,
            imm: 2,
        },
        id: "quest-menu-glyph-site-800342f8",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_44D4,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg::S0,
            rs: Reg::S0,
            imm: 2,
        },
        id: "quest-menu-glyph-site-800344d4",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_4630,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg::S0,
            rs: Reg::S0,
            imm: 2,
        },
        id: "quest-menu-glyph-site-80034630",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_478C,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg::S0,
            rs: Reg::S0,
            imm: 2,
        },
        id: "quest-menu-glyph-site-8003478c",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_4908,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg::S0,
            rs: Reg::S0,
            imm: 2,
        },
        id: "quest-menu-glyph-site-80034908",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_49C0,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg::S1,
            rs: Reg::S1,
            imm: 2,
        },
        id: "quest-menu-glyph-site-800349c0",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_4AA8,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg::S0,
            rs: Reg::S0,
            imm: 2,
        },
        id: "quest-menu-glyph-site-80034aa8",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_4BA8,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg::S0,
            rs: Reg::S0,
            imm: 2,
        },
        id: "quest-menu-glyph-site-80034ba8",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_4DA8,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg::S0,
            rs: Reg::S0,
            imm: 2,
        },
        id: "quest-menu-glyph-site-80034da8",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_52E8,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg::S1,
            rs: Reg::S1,
            imm: 2,
        },
        id: "quest-menu-glyph-site-800352e8",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_537C,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg::S0,
            rs: Reg::S0,
            imm: 2,
        },
        id: "quest-menu-glyph-site-8003537c",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_5464,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg::S0,
            rs: Reg::S0,
            imm: 2,
        },
        id: "quest-menu-glyph-site-80035464",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_54E0,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg::S0,
            rs: Reg::S0,
            imm: 2,
        },
        id: "quest-menu-glyph-site-800354e0",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_55C8,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg::S0,
            rs: Reg::S0,
            imm: 2,
        },
        id: "quest-menu-glyph-site-800355c8",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_5644,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg::S0,
            rs: Reg::S0,
            imm: 2,
        },
        id: "quest-menu-glyph-site-80035644",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_5728,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg::S0,
            rs: Reg::S0,
            imm: 2,
        },
        id: "quest-menu-glyph-site-80035728",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_58D0,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg::S1,
            rs: Reg::S1,
            imm: 2,
        },
        id: "quest-menu-glyph-site-800358d0",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_5AEC,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg::S1,
            rs: Reg::S1,
            imm: 2,
        },
        id: "quest-menu-glyph-site-80035aec",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_5E44,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg::S0,
            rs: Reg::S0,
            imm: 2,
        },
        id: "quest-menu-glyph-site-80035e44",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_5E90,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg::S0,
            rs: Reg::S0,
            imm: 2,
        },
        id: "quest-menu-glyph-site-80035e90",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_65B4,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg::S0,
            rs: Reg::S0,
            imm: 2,
        },
        id: "quest-menu-glyph-site-800365b4",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_664C,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg::S0,
            rs: Reg::S0,
            imm: 2,
        },
        id: "quest-menu-glyph-site-8003664c",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_6724,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg::S0,
            rs: Reg::S0,
            imm: 2,
        },
        id: "quest-menu-glyph-site-80036724",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_67E4,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg::S1,
            rs: Reg::S1,
            imm: 2,
        },
        id: "quest-menu-glyph-site-800367e4",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_687C,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg::S0,
            rs: Reg::S0,
            imm: 2,
        },
        id: "quest-menu-glyph-site-8003687c",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_6C7C,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg::S0,
            rs: Reg::S0,
            imm: 2,
        },
        id: "quest-menu-glyph-site-80036c7c",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_6DC4,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg::S0,
            rs: Reg::S0,
            imm: 2,
        },
        id: "quest-menu-glyph-site-80036dc4",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_6E10,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg::S0,
            rs: Reg::S0,
            imm: 2,
        },
        id: "quest-menu-glyph-site-80036e10",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_6F58,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg::S0,
            rs: Reg::S0,
            imm: 2,
        },
        id: "quest-menu-glyph-site-80036f58",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_6FA4,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg::S0,
            rs: Reg::S0,
            imm: 2,
        },
        id: "quest-menu-glyph-site-80036fa4",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_70E8,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg::S0,
            rs: Reg::S0,
            imm: 2,
        },
        id: "quest-menu-glyph-site-800370e8",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_7C98,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg::S0,
            rs: Reg::S0,
            imm: 2,
        },
        id: "quest-menu-glyph-site-80037c98",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_7CE4,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg::S0,
            rs: Reg::S0,
            imm: 2,
        },
        id: "quest-menu-glyph-site-80037ce4",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_8298,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg::S0,
            rs: Reg::S0,
            imm: 2,
        },
        id: "quest-menu-glyph-site-80038298",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_832C,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg::S0,
            rs: Reg::S0,
            imm: 2,
        },
        id: "quest-menu-glyph-site-8003832c",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_99F4,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg::S0,
            rs: Reg::S0,
            imm: 2,
        },
        id: "quest-menu-glyph-site-800399f4",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_9A3C,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg::S0,
            rs: Reg::S0,
            imm: 2,
        },
        id: "quest-menu-glyph-site-80039a3c",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_9EFC,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg::S0,
            rs: Reg::S0,
            imm: 2,
        },
        id: "quest-menu-glyph-site-80039efc",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_A150,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg::S0,
            rs: Reg::S0,
            imm: 2,
        },
        id: "quest-menu-glyph-site-8003a150",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_A1B0,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg::S0,
            rs: Reg::S0,
            imm: 2,
        },
        id: "quest-menu-glyph-site-8003a1b0",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_A210,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg::S0,
            rs: Reg::S0,
            imm: 2,
        },
        id: "quest-menu-glyph-site-8003a210",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_A284,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg::S0,
            rs: Reg::S0,
            imm: 2,
        },
        id: "quest-menu-glyph-site-8003a284",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_A2F8,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg::S0,
            rs: Reg::S0,
            imm: 2,
        },
        id: "quest-menu-glyph-site-8003a2f8",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_A358,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg::S0,
            rs: Reg::S0,
            imm: 2,
        },
        id: "quest-menu-glyph-site-8003a358",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_B154,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg::S0,
            rs: Reg::S0,
            imm: 2,
        },
        id: "quest-menu-glyph-site-8003b154",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_B1E8,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg::S0,
            rs: Reg::S0,
            imm: 2,
        },
        id: "quest-menu-glyph-site-8003b1e8",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_B408,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg::S0,
            rs: Reg::S0,
            imm: 2,
        },
        id: "quest-menu-glyph-site-8003b408",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_B554,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg::S0,
            rs: Reg::S0,
            imm: 2,
        },
        id: "quest-menu-glyph-site-8003b554",
    },
    QuestMenuGlyphCall {
        runtime: 0x8005_337C,
        whole_string: true,
        delay: K::Addiu {
            rt: Reg::A1,
            rs: Reg::A1,
            imm: 6412,
        },
        id: "quest-menu-glyph-site-8005337c",
    },
    QuestMenuGlyphCall {
        runtime: 0x8005_33B0,
        whole_string: true,
        delay: K::Addiu {
            rt: Reg::A1,
            rs: Reg::A1,
            imm: 6428,
        },
        id: "quest-menu-glyph-site-800533b0",
    },
    QuestMenuGlyphCall {
        runtime: 0x8005_34EC,
        whole_string: true,
        delay: K::Addiu {
            rt: Reg::A1,
            rs: Reg::A1,
            imm: 6444,
        },
        id: "quest-menu-glyph-site-800534ec",
    },
    QuestMenuGlyphCall {
        runtime: 0x8005_3684,
        whole_string: true,
        delay: K::Addiu {
            rt: Reg::A1,
            rs: Reg::A1,
            imm: 6480,
        },
        id: "quest-menu-glyph-site-80053684",
    },
    QuestMenuGlyphCall {
        runtime: 0x8005_36BC,
        whole_string: true,
        delay: K::Addiu {
            rt: Reg::A1,
            rs: Reg::A1,
            imm: 6504,
        },
        id: "quest-menu-glyph-site-800536bc",
    },
    QuestMenuGlyphCall {
        runtime: 0x8005_370C,
        whole_string: true,
        delay: K::Addiu {
            rt: Reg::A1,
            rs: Reg::A1,
            imm: 6520,
        },
        id: "quest-menu-glyph-site-8005370c",
    },
    QuestMenuGlyphCall {
        runtime: 0x8005_3744,
        whole_string: true,
        delay: K::Addiu {
            rt: Reg::A1,
            rs: Reg::A1,
            imm: 6504,
        },
        id: "quest-menu-glyph-site-80053744",
    },
    QuestMenuGlyphCall {
        runtime: 0x8005_3794,
        whole_string: true,
        delay: K::Addiu {
            rt: Reg::A1,
            rs: Reg::A1,
            imm: 6544,
        },
        id: "quest-menu-glyph-site-80053794",
    },
    QuestMenuGlyphCall {
        runtime: 0x8005_37CC,
        whole_string: true,
        delay: K::Addiu {
            rt: Reg::A1,
            rs: Reg::A1,
            imm: 6504,
        },
        id: "quest-menu-glyph-site-800537cc",
    },
    QuestMenuGlyphCall {
        runtime: 0x8005_3944,
        whole_string: true,
        delay: K::Addiu {
            rt: Reg::A1,
            rs: Reg::A1,
            imm: 6572,
        },
        id: "quest-menu-glyph-site-80053944",
    },
    QuestMenuGlyphCall {
        runtime: 0x8005_395C,
        whole_string: true,
        delay: K::Addiu {
            rt: Reg::A1,
            rs: Reg::A1,
            imm: 6596,
        },
        id: "quest-menu-glyph-site-8005395c",
    },
    QuestMenuGlyphCall {
        runtime: 0x8002_D2DC,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg(16),
            rs: Reg(16),
            imm: 2,
        },
        id: "quest-menu-glyph-site-8002d2dc",
    },
    QuestMenuGlyphCall {
        runtime: 0x8002_D320,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg(16),
            rs: Reg(16),
            imm: 2,
        },
        id: "quest-menu-glyph-site-8002d320",
    },
    QuestMenuGlyphCall {
        runtime: 0x8002_D364,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg(16),
            rs: Reg(16),
            imm: 2,
        },
        id: "quest-menu-glyph-site-8002d364",
    },
    QuestMenuGlyphCall {
        runtime: 0x8002_D3A8,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg(16),
            rs: Reg(16),
            imm: 2,
        },
        id: "quest-menu-glyph-site-8002d3a8",
    },
    QuestMenuGlyphCall {
        runtime: 0x8002_D3EC,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg(16),
            rs: Reg(16),
            imm: 2,
        },
        id: "quest-menu-glyph-site-8002d3ec",
    },
    QuestMenuGlyphCall {
        runtime: 0x8002_D430,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg(16),
            rs: Reg(16),
            imm: 2,
        },
        id: "quest-menu-glyph-site-8002d430",
    },
    QuestMenuGlyphCall {
        runtime: 0x8002_D7E4,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg(16),
            rs: Reg(16),
            imm: 2,
        },
        id: "quest-menu-glyph-site-8002d7e4",
    },
    QuestMenuGlyphCall {
        runtime: 0x8002_D830,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg(16),
            rs: Reg(16),
            imm: 2,
        },
        id: "quest-menu-glyph-site-8002d830",
    },
    QuestMenuGlyphCall {
        runtime: 0x8002_D878,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg(16),
            rs: Reg(16),
            imm: 2,
        },
        id: "quest-menu-glyph-site-8002d878",
    },
    QuestMenuGlyphCall {
        runtime: 0x8002_D9F8,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg(16),
            rs: Reg(16),
            imm: 2,
        },
        id: "quest-menu-glyph-site-8002d9f8",
    },
    QuestMenuGlyphCall {
        runtime: 0x8002_DB8C,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg(16),
            rs: Reg(16),
            imm: 2,
        },
        id: "quest-menu-glyph-site-8002db8c",
    },
    QuestMenuGlyphCall {
        runtime: 0x8002_DD20,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg(16),
            rs: Reg(16),
            imm: 2,
        },
        id: "quest-menu-glyph-site-8002dd20",
    },
    QuestMenuGlyphCall {
        runtime: 0x8002_E054,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg(16),
            rs: Reg(16),
            imm: 2,
        },
        id: "quest-menu-glyph-site-8002e054",
    },
    QuestMenuGlyphCall {
        runtime: 0x8002_E0F0,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg(16),
            rs: Reg(16),
            imm: 2,
        },
        id: "quest-menu-glyph-site-8002e0f0",
    },
    QuestMenuGlyphCall {
        runtime: 0x8002_E13C,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg(16),
            rs: Reg(16),
            imm: 2,
        },
        id: "quest-menu-glyph-site-8002e13c",
    },
    QuestMenuGlyphCall {
        runtime: 0x8002_E184,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg(16),
            rs: Reg(16),
            imm: 2,
        },
        id: "quest-menu-glyph-site-8002e184",
    },
    QuestMenuGlyphCall {
        runtime: 0x8002_E3FC,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg(16),
            rs: Reg(16),
            imm: 2,
        },
        id: "quest-menu-glyph-site-8002e3fc",
    },
    QuestMenuGlyphCall {
        runtime: 0x8002_E448,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg(16),
            rs: Reg(16),
            imm: 2,
        },
        id: "quest-menu-glyph-site-8002e448",
    },
    QuestMenuGlyphCall {
        runtime: 0x8002_E4AC,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg(16),
            rs: Reg(16),
            imm: 2,
        },
        id: "quest-menu-glyph-site-8002e4ac",
    },
    QuestMenuGlyphCall {
        runtime: 0x8002_E510,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg(16),
            rs: Reg(16),
            imm: 2,
        },
        id: "quest-menu-glyph-site-8002e510",
    },
    QuestMenuGlyphCall {
        runtime: 0x8002_E55C,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg(16),
            rs: Reg(16),
            imm: 2,
        },
        id: "quest-menu-glyph-site-8002e55c",
    },
    QuestMenuGlyphCall {
        runtime: 0x8002_E5C0,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg(16),
            rs: Reg(16),
            imm: 2,
        },
        id: "quest-menu-glyph-site-8002e5c0",
    },
    QuestMenuGlyphCall {
        runtime: 0x8002_E624,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg(16),
            rs: Reg(16),
            imm: 2,
        },
        id: "quest-menu-glyph-site-8002e624",
    },
    QuestMenuGlyphCall {
        runtime: 0x8002_E6A8,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg(16),
            rs: Reg(16),
            imm: 2,
        },
        id: "quest-menu-glyph-site-8002e6a8",
    },
    QuestMenuGlyphCall {
        runtime: 0x8002_E790,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg(16),
            rs: Reg(16),
            imm: 2,
        },
        id: "quest-menu-glyph-site-8002e790",
    },
    QuestMenuGlyphCall {
        runtime: 0x8002_EA84,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg(16),
            rs: Reg(16),
            imm: 2,
        },
        id: "quest-menu-glyph-site-8002ea84",
    },
    QuestMenuGlyphCall {
        runtime: 0x8002_EAD0,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg(16),
            rs: Reg(16),
            imm: 2,
        },
        id: "quest-menu-glyph-site-8002ead0",
    },
    QuestMenuGlyphCall {
        runtime: 0x8002_EB18,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg(16),
            rs: Reg(16),
            imm: 2,
        },
        id: "quest-menu-glyph-site-8002eb18",
    },
    QuestMenuGlyphCall {
        runtime: 0x8002_ECD8,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg(16),
            rs: Reg(16),
            imm: 2,
        },
        id: "quest-menu-glyph-site-8002ecd8",
    },
    QuestMenuGlyphCall {
        runtime: 0x8002_EE6C,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg(16),
            rs: Reg(16),
            imm: 2,
        },
        id: "quest-menu-glyph-site-8002ee6c",
    },
    QuestMenuGlyphCall {
        runtime: 0x8002_F000,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg(16),
            rs: Reg(16),
            imm: 2,
        },
        id: "quest-menu-glyph-site-8002f000",
    },
    QuestMenuGlyphCall {
        runtime: 0x8002_F318,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg(16),
            rs: Reg(16),
            imm: 2,
        },
        id: "quest-menu-glyph-site-8002f318",
    },
    QuestMenuGlyphCall {
        runtime: 0x8002_F364,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg(16),
            rs: Reg(16),
            imm: 2,
        },
        id: "quest-menu-glyph-site-8002f364",
    },
    QuestMenuGlyphCall {
        runtime: 0x8002_F494,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg(16),
            rs: Reg(16),
            imm: 2,
        },
        id: "quest-menu-glyph-site-8002f494",
    },
    QuestMenuGlyphCall {
        runtime: 0x8002_F4DC,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg(16),
            rs: Reg(16),
            imm: 2,
        },
        id: "quest-menu-glyph-site-8002f4dc",
    },
    QuestMenuGlyphCall {
        runtime: 0x8002_F5A4,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg(16),
            rs: Reg(16),
            imm: 2,
        },
        id: "quest-menu-glyph-site-8002f5a4",
    },
    QuestMenuGlyphCall {
        runtime: 0x8002_F5F0,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg(16),
            rs: Reg(16),
            imm: 2,
        },
        id: "quest-menu-glyph-site-8002f5f0",
    },
    QuestMenuGlyphCall {
        runtime: 0x8002_F638,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg(16),
            rs: Reg(16),
            imm: 2,
        },
        id: "quest-menu-glyph-site-8002f638",
    },
    QuestMenuGlyphCall {
        runtime: 0x8002_F7D0,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg(16),
            rs: Reg(16),
            imm: 2,
        },
        id: "quest-menu-glyph-site-8002f7d0",
    },
    QuestMenuGlyphCall {
        runtime: 0x8002_F9A4,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg(16),
            rs: Reg(16),
            imm: 2,
        },
        id: "quest-menu-glyph-site-8002f9a4",
    },
    QuestMenuGlyphCall {
        runtime: 0x8002_F9F0,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg(16),
            rs: Reg(16),
            imm: 2,
        },
        id: "quest-menu-glyph-site-8002f9f0",
    },
    QuestMenuGlyphCall {
        runtime: 0x8002_FA74,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg(16),
            rs: Reg(16),
            imm: 2,
        },
        id: "quest-menu-glyph-site-8002fa74",
    },
    QuestMenuGlyphCall {
        runtime: 0x8002_FAD8,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg(16),
            rs: Reg(16),
            imm: 2,
        },
        id: "quest-menu-glyph-site-8002fad8",
    },
    QuestMenuGlyphCall {
        runtime: 0x8002_FB24,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg(16),
            rs: Reg(16),
            imm: 2,
        },
        id: "quest-menu-glyph-site-8002fb24",
    },
    QuestMenuGlyphCall {
        runtime: 0x8002_FB88,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg(16),
            rs: Reg(16),
            imm: 2,
        },
        id: "quest-menu-glyph-site-8002fb88",
    },
    QuestMenuGlyphCall {
        runtime: 0x8002_FBD4,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg(16),
            rs: Reg(16),
            imm: 2,
        },
        id: "quest-menu-glyph-site-8002fbd4",
    },
    QuestMenuGlyphCall {
        runtime: 0x8002_FC38,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg(16),
            rs: Reg(16),
            imm: 2,
        },
        id: "quest-menu-glyph-site-8002fc38",
    },
    QuestMenuGlyphCall {
        runtime: 0x8002_FCBC,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg(16),
            rs: Reg(16),
            imm: 2,
        },
        id: "quest-menu-glyph-site-8002fcbc",
    },
    QuestMenuGlyphCall {
        runtime: 0x8002_FE24,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg(16),
            rs: Reg(16),
            imm: 2,
        },
        id: "quest-menu-glyph-site-8002fe24",
    },
    QuestMenuGlyphCall {
        runtime: 0x8002_FE6C,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg(16),
            rs: Reg(16),
            imm: 2,
        },
        id: "quest-menu-glyph-site-8002fe6c",
    },
    QuestMenuGlyphCall {
        runtime: 0x8002_FF10,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg(16),
            rs: Reg(16),
            imm: 2,
        },
        id: "quest-menu-glyph-site-8002ff10",
    },
    QuestMenuGlyphCall {
        runtime: 0x8002_FF5C,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg(16),
            rs: Reg(16),
            imm: 2,
        },
        id: "quest-menu-glyph-site-8002ff5c",
    },
    QuestMenuGlyphCall {
        runtime: 0x8002_FFA8,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg(16),
            rs: Reg(16),
            imm: 2,
        },
        id: "quest-menu-glyph-site-8002ffa8",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_0060,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg(16),
            rs: Reg(16),
            imm: 2,
        },
        id: "quest-menu-glyph-site-80030060",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_00E0,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg(16),
            rs: Reg(16),
            imm: 2,
        },
        id: "quest-menu-glyph-site-800300e0",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_016C,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg(16),
            rs: Reg(16),
            imm: 2,
        },
        id: "quest-menu-glyph-site-8003016c",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_01EC,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg(16),
            rs: Reg(16),
            imm: 2,
        },
        id: "quest-menu-glyph-site-800301ec",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_0234,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg(16),
            rs: Reg(16),
            imm: 2,
        },
        id: "quest-menu-glyph-site-80030234",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_0B48,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg(16),
            rs: Reg(16),
            imm: 2,
        },
        id: "quest-menu-glyph-site-80030b48",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_0B90,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg(16),
            rs: Reg(16),
            imm: 2,
        },
        id: "quest-menu-glyph-site-80030b90",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_0BD8,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg(16),
            rs: Reg(16),
            imm: 2,
        },
        id: "quest-menu-glyph-site-80030bd8",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_0C20,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg(16),
            rs: Reg(16),
            imm: 2,
        },
        id: "quest-menu-glyph-site-80030c20",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_0C68,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg(16),
            rs: Reg(16),
            imm: 2,
        },
        id: "quest-menu-glyph-site-80030c68",
    },
    QuestMenuGlyphCall {
        runtime: 0x8003_0CB0,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg(16),
            rs: Reg(16),
            imm: 2,
        },
        id: "quest-menu-glyph-site-80030cb0",
    },
    QuestMenuGlyphCall {
        runtime: 0x8002_D474,
        whole_string: false,
        delay: K::Addiu {
            rt: Reg::S0,
            rs: Reg::S0,
            imm: 2,
        },
        id: "quest-menu-glyph-site-8002d474",
    },
];

pub(super) fn verify_menu_consumers(map: &ExeMap, text: &[u8]) -> Result<(), QuestUiLiteralError> {
    for &(target, expected) in REFERENCES {
        let actual = address_load_references(map, text, target)?;
        if actual != expected || !raw_pointer_locations(map, text, target).is_empty() {
            return Err(error(format!(
                "QUEST menu literal 참조 drift: {target:08X}, {actual:08X?}"
            )));
        }
    }
    for call in QUEST_MENU_GLYPH_CALLS {
        require_instruction(
            map,
            text,
            call.runtime,
            K::Jal {
                target: if call.whole_string {
                    QUEST_WHOLE_STRING_RENDERER_RUNTIME
                } else {
                    QUEST_GLYPH_RENDERER_RUNTIME
                },
            },
        )?;
        require_instruction(map, text, call.runtime + 4, call.delay)?;
    }
    let instructions =
        decode_slice(map.load_addr, text).ok_or_else(|| error("QUEST menu 명령 정렬"))?;
    let popup_calls = instructions
        .iter()
        .filter_map(|i| {
            (i.kind
                == K::Jal {
                    target: 0x8002_CF98,
                })
            .then_some(i.addr)
        })
        .collect::<Vec<_>>();
    if popup_calls != [0x8003_1F94, 0x8003_2CD4, 0x8003_AE24, 0x8003_B70C] {
        return Err(error("QUEST popup caller 분모 drift"));
    }
    for (high, literal) in [
        (0x8003_1F70, 0x8001_0FE8u32),
        (0x8003_2CB0, 0x8001_0FE8),
        (0x8003_AE04, 0x8001_1200),
        (0x8003_B6EC, 0x8001_1218),
    ] {
        require_instruction(
            map,
            text,
            high,
            K::Lui {
                rt: Reg::A2,
                imm: 0x8001,
            },
        )?;
        require_instruction(
            map,
            text,
            high + 4,
            K::Addiu {
                rt: Reg::A2,
                rs: Reg::A2,
                imm: literal as u16 as i16,
            },
        )?;
    }
    for (at, kind) in [
        (
            0x8002_CFA8,
            K::Addu {
                rd: Reg::S4,
                rs: Reg::A2,
                rt: Reg::ZERO,
            },
        ),
        (
            0x8002_D0A8,
            K::Jal {
                target: QUEST_STRING_LENGTH_RUNTIME,
            },
        ),
        (
            0x8002_D0AC,
            K::Addu {
                rd: Reg::A0,
                rs: Reg::S4,
                rt: Reg::ZERO,
            },
        ),
        (
            0x8002_D0C0,
            K::Addu {
                rd: Reg::V0,
                rs: Reg::S4,
                rt: Reg::S0,
            },
        ),
        (
            0x8002_D0C4,
            K::Lhu {
                rt: Reg::A1,
                base: Reg::V0,
                offset: 0,
            },
        ),
    ] {
        require_instruction(map, text, at, kind)?;
    }
    Ok(())
}
