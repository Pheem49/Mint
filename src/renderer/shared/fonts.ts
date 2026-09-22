// Keep the default UI and code faces self-hosted for offline desktop use.
// Import only the families and weights the initial UI uses; alternative font
// selections fall back to the operating system instead of inflating every
// launch with complete Unicode coverage we do not render by default.
import '@fontsource/prompt/latin-400.css';
import '@fontsource/prompt/thai-400.css';
import '@fontsource/prompt/latin-500.css';
import '@fontsource/prompt/thai-500.css';
import '@fontsource/prompt/latin-600.css';
import '@fontsource/prompt/thai-600.css';
import '@fontsource/prompt/latin-700.css';
import '@fontsource/prompt/thai-700.css';

import '@fontsource/fira-code/latin-400.css';
import '@fontsource/fira-code/symbols2-400.css';
import '@fontsource/fira-code/latin-500.css';
import '@fontsource/fira-code/symbols2-500.css';
import '@fontsource/fira-code/latin-600.css';
import '@fontsource/fira-code/symbols2-600.css';
