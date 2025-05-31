import { createTheme } from "@material-ui/core";
import { indigo } from "@material-ui/core/colors";

export enum Theme {
  Light = "light",
  Dark = "dark",
  Darker = "darker"
}

const baseThemeConfig = {
  spacing: 12, // Increased from default 8px to 12px for more generous spacing
  shape: {
    borderRadius: 12, // More rounded corners (default is 4px)
  },
  typography: {
    fontFamily: '"Inter", "Roboto", "Helvetica", "Arial", sans-serif',
    h1: {
      fontWeight: 600,
      letterSpacing: '-0.02em',
    },
    h2: {
      fontWeight: 600,
      letterSpacing: '-0.01em',
    },
    h3: {
      fontWeight: 600,
      letterSpacing: '-0.01em',
    },
    h4: {
      fontWeight: 600,
    },
    h5: {
      fontWeight: 600,
    },
    h6: {
      fontWeight: 600,
    },
    body1: {
      lineHeight: 1.6,
    },
    body2: {
      lineHeight: 1.5,
    },
    button: {
      fontWeight: 600,
      textTransform: "none" as const, // Prevent all caps
      letterSpacing: '0.02em',
    },
    overline: {
      textTransform: "none" as const, // This prevents the text from being all caps
      fontFamily: "monospace",
      letterSpacing: '0.1em',
    },
  },
  overrides: {
    MuiButton: {
      root: {
        borderRadius: 12,
        padding: '12px 24px',
        boxShadow: 'none',
        '&:hover': {
          boxShadow: '0 4px 12px rgba(0, 0, 0, 0.15)',
        },
      },
      contained: {
        boxShadow: '0 2px 8px rgba(0, 0, 0, 0.1)',
        '&:hover': {
          boxShadow: '0 4px 16px rgba(0, 0, 0, 0.2)',
        },
      },
    },
    MuiCard: {
      root: {
        borderRadius: 16,
        boxShadow: '0 4px 20px rgba(0, 0, 0, 0.08)',
      },
    },
    MuiPaper: {
      root: {
        borderRadius: 12,
      },
      elevation1: {
        boxShadow: '0 2px 8px rgba(0, 0, 0, 0.08)',
      },
      elevation2: {
        boxShadow: '0 4px 12px rgba(0, 0, 0, 0.1)',
      },
      elevation3: {
        boxShadow: '0 6px 16px rgba(0, 0, 0, 0.12)',
      },
    },
    MuiTextField: {
      root: {
        '& .MuiOutlinedInput-root': {
          borderRadius: 12,
        },
      },
    },
    MuiChip: {
      root: {
        borderRadius: 20,
      },
    },
  },
};

const darkTheme = createTheme({
  ...baseThemeConfig,
  palette: {
    type: "dark",
    primary: {
      main: "#f4511e", // Monero orange
      light: "#ff7043",
      dark: "#d84315",
    },
    secondary: indigo,
    background: {
      default: "#121212",
      paper: "#1e1e1e",
    },
    text: {
      primary: "rgba(255, 255, 255, 0.95)",
      secondary: "rgba(255, 255, 255, 0.7)",
    },
  },
});

const lightTheme = createTheme({
  ...baseThemeConfig,
  palette: {
    type: "light",
    primary: {
      main: "#f4511e", // Monero orange
      light: "#ff7043",
      dark: "#d84315",
    },
    secondary: indigo,
    background: {
      default: "#fafafa",
      paper: "#ffffff",
    },
    text: {
      primary: "rgba(0, 0, 0, 0.87)",
      secondary: "rgba(0, 0, 0, 0.6)",
    },
  },
  overrides: {
    ...baseThemeConfig.overrides,
    MuiCard: {
      root: {
        borderRadius: 16,
        boxShadow: '0 2px 12px rgba(0, 0, 0, 0.04)',
        border: '1px solid rgba(0, 0, 0, 0.06)',
      },
    },
    MuiPaper: {
      root: {
        borderRadius: 12,
      },
      elevation1: {
        boxShadow: '0 1px 4px rgba(0, 0, 0, 0.04)',
      },
      elevation2: {
        boxShadow: '0 2px 8px rgba(0, 0, 0, 0.06)',
      },
      elevation3: {
        boxShadow: '0 4px 12px rgba(0, 0, 0, 0.08)',
      },
    },
  },
});

const darkerTheme = createTheme({
  ...baseThemeConfig,
  palette: {
    type: 'dark',
    primary: {
      main: "#f4511e",
      light: "#ff7043",
      dark: "#d84315",
    },
    secondary: indigo,
    background: {
      default: "#080808",
      paper: "#181818",
    },
    text: {
      primary: "rgba(255, 255, 255, 0.95)",
      secondary: "rgba(255, 255, 255, 0.65)",
    },
  },
  overrides: {
    ...baseThemeConfig.overrides,
    MuiCard: {
      root: {
        borderRadius: 16,
        boxShadow: '0 4px 20px rgba(0, 0, 0, 0.3)',
        border: '1px solid rgba(255, 255, 255, 0.08)',
      },
    },
    MuiPaper: {
      root: {
        borderRadius: 12,
        backgroundColor: "#181818",
      },
      elevation1: {
        boxShadow: '0 2px 8px rgba(0, 0, 0, 0.4)',
      },
      elevation2: {
        boxShadow: '0 4px 12px rgba(0, 0, 0, 0.5)',
      },
      elevation3: {
        boxShadow: '0 6px 16px rgba(0, 0, 0, 0.6)',
      },
    },
  },
});

export const themes = {
  [Theme.Dark]: darkTheme,
  [Theme.Light]: lightTheme,
  [Theme.Darker]: darkerTheme,
};
