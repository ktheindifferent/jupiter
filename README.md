## Jupiter Weather Server
A rust-y weather server designed by the Open Sam Foundation and PixelCoda.

## Server Lifecycle and Management

### Configuration

The server uses environment variables for configuration. You can set them directly or use a `.env` file for local development.

#### Required Environment Variables
- `ACCUWEATHERKEY`: Your AccuWeather API key
- `ZIP_CODE`: The ZIP code for weather data (5-digit US ZIP code)

#### Optional Database Configuration
At least one database configuration must be provided:

**Homebrew Database** (for homebrew weather monitoring):
- `HOMEBREW_PG_DBNAME`: Database name
- `HOMEBREW_PG_USER`: Database username
- `HOMEBREW_PG_PASS`: Database password
- `HOMEBREW_PG_ADDRESS`: Database address (defaults to `localhost:5432`)

**Combo Database** (for combo weather provider):
- `COMBO_PG_DBNAME`: Database name
- `COMBO_PG_USER`: Database username
- `COMBO_PG_PASS`: Database password
- `COMBO_PG_ADDRESS`: Database address (defaults to `localhost:5432`)

### Starting the Server

#### Using environment variables:
```bash
export ACCUWEATHERKEY="your_api_key"
export ZIP_CODE="12345"
export COMBO_PG_DBNAME="combo_weather"
export COMBO_PG_USER="combo_user"
export COMBO_PG_PASS="secure_password"
cargo run
```

#### Using .env file (recommended for development):
1. Copy `.env.example` to `.env`
2. Fill in your configuration values
3. Run the server:
```bash
cargo run
```

The server will automatically load the `.env` file if present, with environment variables taking precedence.

### Graceful Shutdown
The server now supports graceful shutdown through signal handling:
- **SIGTERM**: Gracefully stops the server (commonly used in production)
- **SIGINT (Ctrl+C)**: Gracefully stops the server (commonly used during development)

When a shutdown signal is received, the server will:
1. Log the shutdown signal
2. Allow 2 seconds for in-flight requests to complete
3. Exit cleanly with status code 0

### CPU Usage
The infinite loop CPU exhaustion issue has been fixed. The server now uses proper async signal handling with tokio, ensuring minimal CPU usage while waiting for shutdown signals.

## Current Features
* Partial AcuWeather API Support
    * Location API
        * Search by Zip Code
    * Forecast API
        * Get Daily Forecast
    * CurrentConditions API
        * Get Current Conditions
* Homebrew Weather API
    * Ability to POST/GET weather reports from your own equipment
* Combo API
    * Ability to fetch weather data from multiple providers
    * Ability to cache weather data to reduce outside API calls
    
## Roadmap
* Full AcuWeather API support
* Full OpenWeather API support

## License

Released under Apache 2.0 or MIT.

# Support and follow my work by:

#### Buying my dope NTFs:
 * https://opensea.io/accounts/PixelCoda

#### Checking out my Github:
 * https://github.com/PixelCoda

#### Following my facebook page:
 * https://www.facebook.com/pixelcoda/

#### Subscribing to my Patreon:
 * https://www.patreon.com/calebsmith_pixelcoda

#### Or donating crypto:
 * ADA: addr1qyp299a45tgvveh83tcxlf7ds3yaeh969yt3v882lvxfkkv4e0f46qvr4wzj8ty5c05jyffzq8a9pfwz9dl6m0raac7s4rac48
 * ALGO: VQ5EK4GA3IUTGSPNGV64UANBUVFAIVBXVL5UUCNZSDH544XIMF7BAHEDM4
 * ATOM: cosmos1wm7lummcealk0fxn3x9tm8hg7xsyuz06ul5fw9
 * BTC: bc1qh5p3rff4vxnv23vg0hw8pf3gmz3qgc029cekxz
 * ETH: 0x7A66beaebF7D0d17598d37525e63f524CfD23452
 * ERC20: 0x7A66beaebF7D0d17598d37525e63f524CfD23452
 * XLM: GCJAUMCO2L7PTYMXELQ6GHBTF25MCQKEBNSND2C4QMUPTSVCPEN3LCOG
 * XTZ: tz1SgJppPn56whprsDDGcqR4fxqCr2PXvg1R