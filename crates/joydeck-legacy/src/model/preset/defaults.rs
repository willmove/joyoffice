//! Preset fields accepted by the v1 on-disk format (183 names).
//! The names and literal defaults are compatibility data from the frozen v1 catalog,
//! with its provenance in licenses/joyoffice/source-map.json.

pub fn defaults(name: &str) -> Option<&'static [f64]> {
    Some(match name {
        "actionButtonBackPrevious"
        | "actionButtonBeginning"
        | "actionButtonBlank"
        | "actionButtonDocument"
        | "actionButtonEnd"
        | "actionButtonForwardNext"
        | "actionButtonHelp"
        | "actionButtonHome"
        | "actionButtonInformation"
        | "actionButtonMovie"
        | "actionButtonReturn"
        | "actionButtonSound"
        | "bentConnector2"
        | "chartPlus"
        | "chartStar"
        | "chartX"
        | "cloud"
        | "cornerTabs"
        | "decagon"
        | "diamond"
        | "dodecagon"
        | "ellipse"
        | "flowChartAlternateProcess"
        | "flowChartCollate"
        | "flowChartConnector"
        | "flowChartDecision"
        | "flowChartDelay"
        | "flowChartDisplay"
        | "flowChartDocument"
        | "flowChartExtract"
        | "flowChartInputOutput"
        | "flowChartInternalStorage"
        | "flowChartMagneticDisk"
        | "flowChartMagneticDrum"
        | "flowChartMagneticTape"
        | "flowChartManualInput"
        | "flowChartManualOperation"
        | "flowChartMerge"
        | "flowChartMultidocument"
        | "flowChartOfflineStorage"
        | "flowChartOffpageConnector"
        | "flowChartOnlineStorage"
        | "flowChartOr"
        | "flowChartPredefinedProcess"
        | "flowChartPreparation"
        | "flowChartProcess"
        | "flowChartPunchedCard"
        | "flowChartPunchedTape"
        | "flowChartSort"
        | "flowChartSummingJunction"
        | "flowChartTerminator"
        | "funnel"
        | "heart"
        | "heptagon"
        | "irregularSeal1"
        | "irregularSeal2"
        | "lightningBolt"
        | "line"
        | "lineInv"
        | "pentagon"
        | "pieWedge"
        | "plaqueTabs"
        | "rect"
        | "rtTriangle"
        | "squareTabs"
        | "straightConnector1"
        | "textBox" => &[],
        "cloudCallout" | "wedgeEllipseCallout" | "wedgeRectCallout" => &[-20833_f64, 62500_f64],
        "wedgeRoundRectCallout" => &[-20833_f64, 62500_f64, 16667_f64],
        "snip2DiagRect" => &[0_f64, 16667_f64],
        "pie" => &[0_f64, 16200000_f64],
        "smileyFace" => &[4653_f64],
        "doubleWave" => &[6250_f64, 0_f64],
        "bracePair" | "leftBracket" | "rightBracket" => &[8333_f64],
        "leftBrace" | "rightBrace" => &[8333_f64, 50000_f64],
        "gear9" => &[10000_f64, 1763_f64],
        "bevel" | "frame" | "horizontalScroll" | "star4" | "verticalScroll" => &[12500_f64],
        "leftCircularArrow" => &[
            12500_f64,
            -1142319_f64,
            1142319_f64,
            10800000_f64,
            12500_f64,
        ],
        "wave" => &[12500_f64, 0_f64],
        "circularArrow" => &[
            12500_f64,
            1142319_f64,
            20457681_f64,
            10800000_f64,
            12500_f64,
        ],
        "leftRightCircularArrow" => &[
            12500_f64,
            1142319_f64,
            20457681_f64,
            11942319_f64,
            12500_f64,
        ],
        "gear6" => &[15000_f64, 3526_f64],
        "bracketPair" | "foldedCorner" | "plaque" | "round1Rect" | "roundRect" | "snip1Rect" => {
            &[16667_f64]
        }
        "round2DiagRect" | "round2SameRect" | "snip2SameRect" => &[16667_f64, 0_f64],
        "snipRoundRect" => &[16667_f64, 16667_f64],
        "ribbon" | "ribbon2" => &[16667_f64, 50000_f64],
        "quadArrowCallout" => &[18515_f64, 18515_f64, 18515_f64, 48123_f64],
        "noSmoking" => &[18750_f64],
        "accentBorderCallout3" | "accentCallout3" | "borderCallout3" | "callout3" => &[
            18750_f64, -8333_f64, 18750_f64, -16667_f64, 100000_f64, -16667_f64, 112963_f64,
            -8333_f64,
        ],
        "accentBorderCallout2" | "accentCallout2" | "borderCallout2" | "callout2" => &[
            18750_f64, -8333_f64, 18750_f64, -16667_f64, 112500_f64, -46667_f64,
        ],
        "accentBorderCallout1" | "accentCallout1" | "borderCallout1" | "callout1" => {
            &[18750_f64, -8333_f64, 112500_f64, -38333_f64]
        }
        "star5" => &[19098_f64],
        "quadArrow" => &[22500_f64, 22500_f64, 22500_f64],
        "mathMinus" | "mathMultiply" | "mathPlus" => &[23520_f64],
        "mathDivide" => &[23520_f64, 5880_f64, 11760_f64],
        "mathEqual" => &[23520_f64, 11760_f64],
        "mathNotEqual" => &[23520_f64, 6600000_f64, 11760_f64],
        "can" | "cube" | "donut" | "hexagon" | "parallelogram" | "plus" | "sun" | "trapezoid" => {
            &[25000_f64]
        }
        "swooshArrow" => &[25000_f64, 16667_f64],
        "nonIsoscelesTrapezoid" => &[25000_f64, 25000_f64],
        "bentUpArrow" | "leftRightUpArrow" | "leftUpArrow" => &[25000_f64, 25000_f64, 25000_f64],
        "bentArrow" => &[25000_f64, 25000_f64, 25000_f64, 43750_f64],
        "uturnArrow" => &[25000_f64, 25000_f64, 25000_f64, 43750_f64, 75000_f64],
        "leftRightArrowCallout" | "upDownArrowCallout" => {
            &[25000_f64, 25000_f64, 25000_f64, 48123_f64]
        }
        "downArrowCallout" | "leftArrowCallout" | "rightArrowCallout" | "upArrowCallout" => {
            &[25000_f64, 25000_f64, 25000_f64, 64977_f64]
        }
        "ellipseRibbon" | "ellipseRibbon2" => &[25000_f64, 50000_f64, 12500_f64],
        "curvedDownArrow" | "curvedLeftArrow" | "curvedRightArrow" | "curvedUpArrow" => {
            &[25000_f64, 50000_f64, 25000_f64]
        }
        "star6" => &[28868_f64],
        "octagon" => &[29289_f64],
        "halfFrame" => &[33333_f64, 33333_f64],
        "star7" => &[34601_f64],
        "star12" | "star16" | "star24" | "star32" | "star8" => &[37500_f64],
        "star10" => &[42533_f64],
        "bentConnector3" | "chevron" | "curvedConnector3" | "diagStripe" | "homePlate" | "moon"
        | "triangle" => &[50000_f64],
        "corner" | "downArrow" | "leftArrow" | "leftRightArrow" | "notchedRightArrow"
        | "rightArrow" | "stripedRightArrow" | "upArrow" | "upDownArrow" => &[50000_f64, 50000_f64],
        "leftRightRibbon" => &[50000_f64, 50000_f64, 16667_f64],
        "teardrop" => &[100000_f64],
        "chord" => &[2700000_f64, 16200000_f64],
        "blockArc" => &[10800000_f64, 0_f64, 25000_f64],
        "arc" => &[16200000_f64, 0_f64],
        _ => return None,
    })
}
