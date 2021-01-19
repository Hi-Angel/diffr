use super::AppConfig;
use super::LineNumberStyle;
use clap::App;
use clap::AppSettings;
use clap::Arg;
use clap::ArgMatches;
use std::fmt::Display;
use std::fmt::Error as FmtErr;
use std::fmt::Formatter;
use std::str::FromStr;
use termcolor::Color;
use termcolor::ColorSpec;
use termcolor::ParseColorError;

const ABOUT: &str = "
diffr adds word-level diff on top of unified diffs.
word-level diff information is displayed using text attributes.";

const USAGE: &str = "\
diffr reads from standard input and write to standard output.

    Typical usage is for interactive use of diff:
    diff -u <file1> <file2> | diffr
    git show | diffr";

const FLAG_DEBUG: &str = "--debug";
const FLAG_HTML: &str = "--html";
const FLAG_COLOR: &str = "--colors";
const FLAG_LINE_NUMBERS: &str = "--line-numbers";

#[derive(Debug, Clone, Copy)]
enum FaceName {
    Added,
    RefineAdded,
    Removed,
    RefineRemoved,
}

impl EnumString for FaceName {
    fn data() -> &'static [(&'static str, Self)] {
        use FaceName::*;
        &[
            ("added", Added),
            ("refine-added", RefineAdded),
            ("removed", Removed),
            ("refine-removed", RefineRemoved),
        ]
    }
}

impl Display for FaceName {
    fn fmt(&self, f: &mut Formatter) -> Result<(), FmtErr> {
        use FaceName::*;
        match self {
            Added => write!(f, "added"),
            RefineAdded => write!(f, "refine-added"),
            Removed => write!(f, "removed"),
            RefineRemoved => write!(f, "refine-removed"),
        }
    }
}

impl FaceName {
    fn get_face_mut<'a, 'b>(&'a self, config: &'b mut super::AppConfig) -> &'b mut ColorSpec {
        use FaceName::*;
        match self {
            Added => &mut config.added_face,
            RefineAdded => &mut config.refine_added_face,
            Removed => &mut config.removed_face,
            RefineRemoved => &mut config.refine_removed_face,
        }
    }
}

// custom parsing of Option<Color>
struct ColorOpt(Option<Color>);

impl FromStr for ColorOpt {
    type Err = ArgParsingError;
    fn from_str(input: &str) -> Result<Self, Self::Err> {
        if input == "none" {
            Ok(ColorOpt(None))
        } else {
            match input.parse() {
                Ok(color) => Ok(ColorOpt(Some(color))),
                Err(err) => Err(ArgParsingError::Color(err)),
            }
        }
    }
}

trait EnumString: Copy {
    fn data() -> &'static [(&'static str, Self)];
}

fn tryparse<T>(input: &str) -> Result<T, String>
where
    T: EnumString + 'static,
{
    T::data()
        .iter()
        .find(|p| p.0 == input)
        .map(|&p| p.1)
        .ok_or_else(|| {
            format!(
                "got '{}', expected {}",
                input,
                T::data().iter().map(|p| p.0).collect::<Vec<_>>().join("|")
            )
        })
}

#[derive(Debug, Clone, Copy)]
struct LineNumberStyleOpt(LineNumberStyle);

impl EnumString for LineNumberStyleOpt {
    fn data() -> &'static [(&'static str, Self)] {
        use LineNumberStyle::*;
        &[
            ("aligned", LineNumberStyleOpt(Aligned)),
            ("compact", LineNumberStyleOpt(Compact)),
        ]
    }
}

#[derive(Debug, Clone, Copy)]
enum FaceColor {
    Foreground,
    Background,
}

#[derive(Debug, Clone, Copy)]
enum AttributeName {
    Color(FaceColor),
    Italic(bool),
    Bold(bool),
    Intense(bool),
    Underline(bool),
    Reset,
}

impl EnumString for AttributeName {
    fn data() -> &'static [(&'static str, Self)] {
        use AttributeName::*;
        &[
            ("foreground", Color(FaceColor::Foreground)),
            ("background", Color(FaceColor::Background)),
            ("italic", Italic(true)),
            ("noitalic", Italic(false)),
            ("bold", Bold(true)),
            ("nobold", Bold(false)),
            ("intense", Intense(true)),
            ("nointense", Intense(false)),
            ("underline", Underline(true)),
            ("nounderline", Underline(false)),
            ("none", Reset),
        ]
    }
}

#[derive(Debug)]
enum ArgParsingError {
    FaceName(String),
    AttributeName(String),
    Color(ParseColorError),
    MissingValue(FaceName),
    LineNumberStyle(String),
}

impl Display for ArgParsingError {
    fn fmt(&self, f: &mut Formatter) -> Result<(), FmtErr> {
        match self {
            ArgParsingError::FaceName(err) => write!(f, "unexpected face name: {}", err),
            ArgParsingError::AttributeName(err) => write!(f, "unexpected attribute name: {}", err),
            ArgParsingError::Color(err) => write!(f, "unexpected color value: {}", err),
            ArgParsingError::MissingValue(face_name) => write!(
                f,
                "error parsing color: missing color value for face '{}'",
                face_name
            ),
            ArgParsingError::LineNumberStyle(err) => {
                write!(f, "unexpected line number style: {}", err)
            }
        }
    }
}

impl FromStr for FaceName {
    type Err = ArgParsingError;
    fn from_str(input: &str) -> Result<Self, Self::Err> {
        tryparse(input).map_err(ArgParsingError::FaceName)
    }
}

impl FromStr for AttributeName {
    type Err = ArgParsingError;
    fn from_str(input: &str) -> Result<Self, Self::Err> {
        tryparse(input).map_err(ArgParsingError::AttributeName)
    }
}

impl FromStr for LineNumberStyleOpt {
    type Err = ArgParsingError;
    fn from_str(input: &str) -> Result<Self, Self::Err> {
        tryparse(input).map_err(ArgParsingError::LineNumberStyle)
    }
}

fn ignore<T>(_: T) {}

fn parse_line_number_style<'a, Values>(
    config: &mut AppConfig,
    values: Values,
) -> Result<(), ArgParsingError>
where
    Values: Iterator<Item = &'a str>,
{
    let style = if let Some(style) = values.last() {
        style.parse::<LineNumberStyleOpt>()?.0
    } else {
        LineNumberStyle::Compact
    };
    config.line_numbers_style = Some(style);
    Ok(())
}

fn parse_color_attributes<'a, Values>(
    config: &mut AppConfig,
    mut values: Values,
    face_name: FaceName,
) -> Result<(), ArgParsingError>
where
    Values: Iterator<Item = &'a str>,
{
    use AttributeName::*;
    let face = face_name.get_face_mut(config);
    while let Some(value) = values.next() {
        let attribute_name = value.parse::<AttributeName>()?;
        match attribute_name {
            Color(kind) => {
                if let Some(value) = values.next() {
                    let ColorOpt(color) = value.parse::<ColorOpt>()?;
                    match kind {
                        FaceColor::Foreground => face.set_fg(color),
                        FaceColor::Background => face.set_bg(color),
                    };
                } else {
                    return Err(ArgParsingError::MissingValue(face_name));
                }
            }
            Italic(italic) => ignore(face.set_italic(italic)),
            Bold(bold) => ignore(face.set_bold(bold)),
            Intense(intense) => ignore(face.set_intense(intense)),
            Underline(underline) => ignore(face.set_underline(underline)),
            Reset => *face = Default::default(),
        }
    }
    Ok(())
}

fn parse_color_args<'a, Values>(
    config: &mut AppConfig,
    values: Values,
) -> Result<(), ArgParsingError>
where
    Values: Iterator<Item = &'a str>,
{
    for value in values {
        let mut pieces = value.split(':');
        if let Some(piece) = pieces.next() {
            let face_name = piece.parse::<FaceName>()?;
            parse_color_attributes(config, pieces, face_name)?;
        }
    }
    Ok(())
}

fn get_matches() -> ArgMatches<'static> {
    App::new("diffr")
        .setting(AppSettings::UnifiedHelpMessage)
        .version("0.1.4")
        .author("Nathan Moreau <nathan.moreau@m4x.org>")
        .about(ABOUT)
        .usage(USAGE)
        .arg(Arg::with_name(FLAG_DEBUG).long(FLAG_DEBUG).hidden(true))
        .arg(Arg::with_name(FLAG_HTML).long(FLAG_HTML).hidden(true))
        .arg(
            Arg::with_name(FLAG_COLOR)
                .long(FLAG_COLOR)
                .value_name("COLOR_SPEC")
                .takes_value(true)
                .multiple(true)
                .number_of_values(1)
                .help("Configure color settings.")
                .long_help(
                    "Configure color settings for console ouput.

There are four faces to customize:
+----------------+--------------+----------------+
|  line prefix   |      +       |       -        |
+----------------+--------------+----------------+
| common segment |    added     |    removed     |
| unique segment | refine-added | refine-removed |
+----------------+--------------+----------------+

The customization allows
- to change the foreground or background color;
- to set or unset the attributes 'bold', 'intense', 'underline';
- to clear all attributes.

Customization is done passing a color_spec argument.
This flag may be provided multiple times.

The syntax is the following:

color_spec = face-name + ':' + attributes
attributes = attribute
           | attribute + ':' + attributes
attribute  = ('foreground' | 'background') + ':' + color
           | (<empty> | 'no') + font-flag
           | 'none'
font-flag  = 'italic'
           | 'bold'
           | 'intense'
           | 'underline'
color      = 'none'
           | [0-255]
           | [0-255] + ',' + [0-255] + ',' + [0-255]
           | ('black', 'blue', 'green', 'red',
              'cyan', 'magenta', 'yellow', 'white')

For example, the color_spec

    'refine-added:background:blue:bold'

sets the color of unique added segments with
a blue background, written with a bold font.",
                ),
        )
        .arg(
            Arg::with_name(FLAG_LINE_NUMBERS)
                .long(FLAG_LINE_NUMBERS)
                .value_name("compact|aligned")
                .default_value("compact")
                .help("Display line numbers. Style is optional.")
                .long_help(
                    "Display line numbers. Style is optional.
When style = 'compact', take as little width as possible.
When style = 'aligned', align to tab stops (useful if tab is used for indentation).",
                ),
        )
        .get_matches()
}

fn die(err: ArgParsingError) -> ! {
    eprintln!("{}", err);
    std::process::exit(-1)
}

pub fn parse_config() -> AppConfig {
    let matches = get_matches();
    if atty::is(atty::Stream::Stdin) {
        eprintln!("{}", matches.usage());
        std::process::exit(-1)
    }

    let mut config = AppConfig::default();
    config.debug = matches.is_present(FLAG_DEBUG);
    config.html = matches.is_present(FLAG_HTML);
    if matches.occurrences_of(FLAG_LINE_NUMBERS) != 0 {
        if let Some(values) = matches.values_of(FLAG_LINE_NUMBERS) {
            if let Err(err) = parse_line_number_style(&mut config, values) {
                die(err);
            }
        }
    };

    if let Some(values) = matches.values_of(FLAG_COLOR) {
        if let Err(err) = parse_color_args(&mut config, values) {
            die(err);
        }
    }
    config
}
