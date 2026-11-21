package com.vexio.ui.theme

import android.os.Build
import androidx.compose.animation.core.FiniteAnimationSpec
import androidx.compose.animation.core.SpringSpec
import androidx.compose.animation.core.animateDpAsState
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.spring
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.ColorScheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Shapes
import androidx.compose.material3.Typography
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.dynamicDarkColorScheme
import androidx.compose.material3.dynamicLightColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.State
import androidx.compose.runtime.getValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/**
 * Material 3 Expressive motion physics tokens for Vexio Workstation.
 *
 * Uses under-damped spatial springs (`dampingRatio = 0.68f, stiffness = 380f`) for tactile,
 * physical responsiveness on floating toolbars, pane reflows, and pill-to-squircle morphs,
 * paired with critically-damped effects springs (`dampingRatio = 1.0f`) for color/elevation.
 */
object VexioExpressiveMotion {
    const val EXPRESSIVE_DAMPING_RATIO: Float = 0.68f
    const val EXPRESSIVE_STIFFNESS: Float = 380f

    fun <T> spatialSpring(): SpringSpec<T> = spring(
        dampingRatio = EXPRESSIVE_DAMPING_RATIO,
        stiffness = EXPRESSIVE_STIFFNESS
    )

    val floatMorphSpring: SpringSpec<Float> = spring(
        dampingRatio = EXPRESSIVE_DAMPING_RATIO,
        stiffness = EXPRESSIVE_STIFFNESS
    )

    val dpMorphSpring: SpringSpec<Dp> = spring(
        dampingRatio = EXPRESSIVE_DAMPING_RATIO,
        stiffness = EXPRESSIVE_STIFFNESS
    )

    val sizeReflowSpring: FiniteAnimationSpec<IntSize> = spring(
        dampingRatio = EXPRESSIVE_DAMPING_RATIO,
        stiffness = EXPRESSIVE_STIFFNESS
    )

    fun <T> crispEffectsSpring(): SpringSpec<T> = spring(
        dampingRatio = 1.0f,
        stiffness = 620f
    )
}

/**
 * High-contrast domain-specific semantic tokens for Visio ShapeSheet, CAD, and Glue feedback.
 */
@Immutable
data class VexioSemanticColors(
    val canvasBackdrop: Color,
    val pageSheetSurface: Color,
    val gridMajorLine: Color,
    val gridMinorLine: Color,
    val glueSnapAccent: Color,
    val glueSnapContainer: Color,
    val guardBadgeForeground: Color,
    val guardBadgeContainer: Color,
    val setfOverrideAccent: Color,
    val stylusInkAccent: Color,
    val stylusInkContainer: Color,
    val dropZoneHighlightBorder: Color,
    val dropZoneHighlightFill: Color,
    val validationPassForeground: Color,
    val validationPassContainer: Color,
    val formulaSyntaxKeyword: Color,
    val formulaSyntaxUnit: Color
)

private val LightVexioSemanticColors = VexioSemanticColors(
    canvasBackdrop = Color(0xFFE9EEF6),
    pageSheetSurface = Color(0xFFFFFFFF),
    gridMajorLine = Color(0xFFC7D3E6),
    gridMinorLine = Color(0xFFE3EAF4),
    glueSnapAccent = Color(0xFF0B8043),
    glueSnapContainer = Color(0xFFCEEAD6),
    guardBadgeForeground = Color(0xFF8A4B00),
    guardBadgeContainer = Color(0xFFFFEFC3),
    setfOverrideAccent = Color(0xFFB31412),
    stylusInkAccent = Color(0xFF681DA8),
    stylusInkContainer = Color(0xFFF3E8FD),
    dropZoneHighlightBorder = Color(0xFF1A73E8),
    dropZoneHighlightFill = Color(0x241A73E8),
    validationPassForeground = Color(0xFF137333),
    validationPassContainer = Color(0xFFE6F4EA),
    formulaSyntaxKeyword = Color(0xFF185ABC),
    formulaSyntaxUnit = Color(0xFF0D652D)
)

private val DarkVexioSemanticColors = VexioSemanticColors(
    canvasBackdrop = Color(0xFF0E131B),
    pageSheetSurface = Color(0xFF18202C),
    gridMajorLine = Color(0xFF2B384B),
    gridMinorLine = Color(0xFF1E2837),
    glueSnapAccent = Color(0xFF81C995),
    glueSnapContainer = Color(0xFF133824),
    guardBadgeForeground = Color(0xFFFDD663),
    guardBadgeContainer = Color(0xFF423000),
    setfOverrideAccent = Color(0xFFF28B82),
    stylusInkAccent = Color(0xFFD7AEFB),
    stylusInkContainer = Color(0xFF39185A),
    dropZoneHighlightBorder = Color(0xFF8AB4F8),
    dropZoneHighlightFill = Color(0x2E8AB4F8),
    validationPassForeground = Color(0xFF81C995),
    validationPassContainer = Color(0xFF163624),
    formulaSyntaxKeyword = Color(0xFF8AB4F8),
    formulaSyntaxUnit = Color(0xFF81C995)
)

val LocalVexioSemanticColors = staticCompositionLocalOf { LightVexioSemanticColors }

private val VexioExpressiveLightColorScheme: ColorScheme = lightColorScheme(
    primary = Color(0xFF175CD3),
    onPrimary = Color(0xFFFFFFFF),
    primaryContainer = Color(0xFFD6E4FF),
    onPrimaryContainer = Color(0xFF001B3D),
    secondary = Color(0xFF344966),
    onSecondary = Color(0xFFFFFFFF),
    secondaryContainer = Color(0xFFD8E3F8),
    onSecondaryContainer = Color(0xFF0E1D33),
    tertiary = Color(0xFF6B2F9E),
    onTertiary = Color(0xFFFFFFFF),
    tertiaryContainer = Color(0xFFF2DAFF),
    onTertiaryContainer = Color(0xFF260049),
    error = Color(0xFFBA1A1A),
    onError = Color(0xFFFFFFFF),
    errorContainer = Color(0xFFFFDAD6),
    onErrorContainer = Color(0xFF410002),
    background = Color(0xFFF6F8FC),
    onBackground = Color(0xFF181C22),
    surface = Color(0xFFF8FAFE),
    onSurface = Color(0xFF181C22),
    surfaceVariant = Color(0xFFDFE3EC),
    onSurfaceVariant = Color(0xFF424750),
    surfaceContainerLowest = Color(0xFFFFFFFF),
    surfaceContainerLow = Color(0xFFF1F4FA),
    surfaceContainer = Color(0xFFEBEEF5),
    surfaceContainerHigh = Color(0xFFE5E9F0),
    surfaceContainerHighest = Color(0xFFDFE3EA),
    outline = Color(0xFF727781),
    outlineVariant = Color(0xFFC2C7D1)
)

private val VexioExpressiveDarkColorScheme: ColorScheme = darkColorScheme(
    primary = Color(0xFFA6C8FF),
    onPrimary = Color(0xFF003062),
    primaryContainer = Color(0xFF00468A),
    onPrimaryContainer = Color(0xFFD6E4FF),
    secondary = Color(0xFFBCC7DC),
    onSecondary = Color(0xFF263141),
    secondaryContainer = Color(0xFF3C4758),
    onSecondaryContainer = Color(0xFFD8E3F8),
    tertiary = Color(0xFFDFB7FF),
    onTertiary = Color(0xFF41126D),
    tertiaryContainer = Color(0xFF592D86),
    onTertiaryContainer = Color(0xFFF2DAFF),
    error = Color(0xFFFFB4AB),
    onError = Color(0xFF690005),
    errorContainer = Color(0xFF93000A),
    onErrorContainer = Color(0xFFFFDAD6),
    background = Color(0xFF10141A),
    onBackground = Color(0xFFE0E2E9),
    surface = Color(0xFF10141A),
    onSurface = Color(0xFFE0E2E9),
    surfaceVariant = Color(0xFF424750),
    onSurfaceVariant = Color(0xFFC2C7D1),
    surfaceContainerLowest = Color(0xFF0B0E14),
    surfaceContainerLow = Color(0xFF181C22),
    surfaceContainer = Color(0xFF1C2027),
    surfaceContainerHigh = Color(0xFF262A32),
    surfaceContainerHighest = Color(0xFF31353D),
    outline = Color(0xFF8C919B),
    outlineVariant = Color(0xFF424750)
)

val VexioExpressiveShapes = Shapes(
    extraSmall = RoundedCornerShape(8.dp),
    small = RoundedCornerShape(12.dp),
    medium = RoundedCornerShape(18.dp),
    large = RoundedCornerShape(24.dp),
    extraLarge = RoundedCornerShape(32.dp)
)

/**
 * Animates a Material 3 Expressive container shape between a full floating capsule pill (`28.dp`)
 * and a docked desktop workstation squircle (`16.dp`) using `spring(dampingRatio = 0.68f, stiffness = 380f)`.
 */
@Composable
fun rememberMorphingToolbarShape(isFloatingPill: Boolean): Shape {
    val targetRadius = if (isFloatingPill) 28.dp else 16.dp
    val animatedRadius by animateDpAsState(
        targetValue = targetRadius,
        animationSpec = VexioExpressiveMotion.dpMorphSpring,
        label = "ToolbarPillToSquircleMorph"
    )
    return RoundedCornerShape(animatedRadius)
}

/**
 * Animates individual toolbar action buttons between an unselected circular pill (`22.dp`)
 * and an active/selected expressive squircle (`12.dp`), providing unmistakable tactile state feedback.
 */
@Composable
fun rememberExpressiveToolButtonShape(isSelected: Boolean, isPressed: Boolean = false): Shape {
    val targetRadius = when {
        isPressed -> 8.dp
        isSelected -> 12.dp
        else -> 22.dp
    }
    val animatedRadius by animateDpAsState(
        targetValue = targetRadius,
        animationSpec = VexioExpressiveMotion.dpMorphSpring,
        label = "ToolButtonExpressiveMorph"
    )
    return RoundedCornerShape(animatedRadius)
}

/**
 * Animates subtle expressive scale emphasis on active/hovered workstation controls.
 */
@Composable
fun rememberExpressiveScaleState(emphasized: Boolean): State<Float> = animateFloatAsState(
    targetValue = if (emphasized) 1.04f else 1.0f,
    animationSpec = VexioExpressiveMotion.floatMorphSpring,
    label = "ExpressiveScaleState"
)

/**
 * Adaptive typography scaling slightly upward on ultra-wide Googlebook workstation monitors (>= 1200dp)
 * while maintaining dense, crisp metrics for ShapeSheet formula bars and Inspector rows.
 */
@Composable
fun rememberAdaptiveWorkstationTypography(): Typography {
    val screenWidthDp = LocalConfiguration.current.screenWidthDp
    val scaleFactor = when {
        screenWidthDp >= 1600 -> 1.05f
        screenWidthDp < 720 -> 0.96f
        else -> 1.0f
    }
    return Typography(
        headlineSmall = TextStyle(
            fontFamily = FontFamily.SansSerif,
            fontWeight = FontWeight.SemiBold,
            fontSize = (22f * scaleFactor).sp,
            lineHeight = (28f * scaleFactor).sp,
            letterSpacing = (-0.2).sp
        ),
        titleMedium = TextStyle(
            fontFamily = FontFamily.SansSerif,
            fontWeight = FontWeight.SemiBold,
            fontSize = (15f * scaleFactor).sp,
            lineHeight = (21f * scaleFactor).sp,
            letterSpacing = 0.1.sp
        ),
        titleSmall = TextStyle(
            fontFamily = FontFamily.SansSerif,
            fontWeight = FontWeight.SemiBold,
            fontSize = (13f * scaleFactor).sp,
            lineHeight = (18f * scaleFactor).sp,
            letterSpacing = 0.1.sp
        ),
        bodyMedium = TextStyle(
            fontFamily = FontFamily.SansSerif,
            fontWeight = FontWeight.Normal,
            fontSize = (13f * scaleFactor).sp,
            lineHeight = (19f * scaleFactor).sp
        ),
        bodySmall = TextStyle(
            fontFamily = FontFamily.Monospace,
            fontWeight = FontWeight.Medium,
            fontSize = (12f * scaleFactor).sp,
            lineHeight = (17f * scaleFactor).sp,
            fontFeatureSettings = "tnum"
        ),
        labelLarge = TextStyle(
            fontFamily = FontFamily.SansSerif,
            fontWeight = FontWeight.SemiBold,
            fontSize = (13f * scaleFactor).sp,
            lineHeight = (18f * scaleFactor).sp,
            letterSpacing = 0.15.sp
        ),
        labelMedium = TextStyle(
            fontFamily = FontFamily.SansSerif,
            fontWeight = FontWeight.Medium,
            fontSize = (11.5f * scaleFactor).sp,
            lineHeight = (15f * scaleFactor).sp,
            letterSpacing = 0.25.sp
        ),
        labelSmall = TextStyle(
            fontFamily = FontFamily.Monospace,
            fontWeight = FontWeight.Medium,
            fontSize = (11f * scaleFactor).sp,
            lineHeight = (14f * scaleFactor).sp,
            fontFeatureSettings = "tnum"
        )
    )
}

@Composable
fun VexioMaterial3ExpressiveTheme(
    darkTheme: Boolean = false,
    dynamicColor: Boolean = false,
    content: @Composable () -> Unit
) {
    val context = LocalContext.current
    val baseScheme = when {
        dynamicColor && Build.VERSION.SDK_INT >= Build.VERSION_CODES.S -> {
            if (darkTheme) dynamicDarkColorScheme(context) else dynamicLightColorScheme(context)
        }
        darkTheme -> VexioExpressiveDarkColorScheme
        else -> VexioExpressiveLightColorScheme
    }

    val semanticColors = if (darkTheme) DarkVexioSemanticColors else LightVexioSemanticColors
    val adaptiveTypography = rememberAdaptiveWorkstationTypography()

    CompositionLocalProvider(LocalVexioSemanticColors provides semanticColors) {
        MaterialTheme(
            colorScheme = baseScheme,
            shapes = VexioExpressiveShapes,
            typography = adaptiveTypography,
            content = content
        )
    }
}
