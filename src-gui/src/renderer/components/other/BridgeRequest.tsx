import React, { useEffect, useState } from 'react';
import {
    Box,
    Button,
    TextField,
    Typography,
    CircularProgress,
    Radio,
    RadioGroup,
    FormControlLabel,
    DialogActions,
    Dialog,
    DialogContent,
    makeStyles
} from '@material-ui/core';
import { fetch } from "@tauri-apps/plugin-http"

const BASE_URL = 'https://bridges.torproject.org/moat';
const API_VERSION = '0.1.0';
const SUPPORTED_TRANSPORTS = ['obfs4'];

interface CaptchaResponse {
    data: [{
        id: string;
        type: string;
        version: string;
        transport: string;
        image: string;
        challenge: string;
    }];
}

interface BridgeResponse {
    data: [{
        id: string;
        type: string;
        version: string;
        bridges: string[] | null;
        qrcode: string | null;
    }];
}

interface BridgeRequestProps {
    open: boolean;
    onClose: () => void;
    onSubmit: (bridge: string) => void;
}

const useStyles = makeStyles((theme) => ({
    container: {
        padding: theme.spacing(2),
        display: 'flex',
        flexDirection: 'column',
        gap: theme.spacing(2),
    },
    bridgeText: {
        fontFamily: 'monospace',
        fontSize: '0.85rem',
        wordBreak: 'break-all',
    },
    bridgeOption: {
        marginBottom: theme.spacing(2),
        '& .MuiFormControlLabel-label': {
            width: '100%',
        },
        '& .MuiRadio-root': {
            padding: theme.spacing(1),
        }
    },
    error: {
        color: theme.palette.error.main,
        backgroundColor: theme.palette.error.light,
        padding: theme.spacing(1),
        borderRadius: theme.shape.borderRadius,
    },
    captchaImage: {
        maxWidth: '100%',
        borderRadius: theme.shape.borderRadius,
    },
    radioGroup: {
        marginTop: theme.spacing(1),
    }
}));

// API Functions
async function fetchCaptcha(): Promise<{ image: string; challenge: string }> {
    const response = await fetch(`${BASE_URL}/fetch`, {
        method: 'POST',
        headers: {
            'Content-Type': 'application/vnd.api+json',
        },
        body: JSON.stringify({
            data: [{
                version: API_VERSION,
                type: 'client-transports',
                supported: SUPPORTED_TRANSPORTS,
            }]
        })
    });

    if (!response.ok) {
        throw new Error(`Failed to fetch CAPTCHA: ${response.status}`);
    }

    const data: CaptchaResponse = await response.json();
    return {
        image: data.data[0].image,
        challenge: data.data[0].challenge,
    };
}

async function checkCaptcha(challenge: string, solution: string): Promise<string[]> {
    const response = await fetch(`${BASE_URL}/check`, {
        method: 'POST',
        headers: {
            'Content-Type': 'application/vnd.api+json',
        },
        body: JSON.stringify({
            data: [{
                id: "2",
                type: "moat-solution",
                version: API_VERSION,
                transport: SUPPORTED_TRANSPORTS[0],
                challenge: challenge,
                solution: solution,
                qrcode: "true"
            }]
        })
    });

    if (!response.ok) {
        throw new Error(`Failed to verify CAPTCHA: ${response.status}`);
    }

    const data: BridgeResponse = await response.json();
    if (!data.data[0].bridges) {
        throw new Error('No bridges received');
    }

    return data.data[0].bridges;
}

export default function BridgeRequest({ open, onSubmit, onClose }: BridgeRequestProps) {
    const classes = useStyles();
    const [loading, setLoading] = useState<boolean>(true);
    const [error, setError] = useState<string | null>(null);
    const [captcha, setCaptcha] = useState<{ image: string; challenge: string } | null>(null);
    const [solution, setSolution] = useState('');
    const [bridges, setBridges] = useState<string[]>([]);
    const [selectedBridge, setSelectedBridge] = useState<string | null>(null);

    useEffect(() => {
        if (open) {
            setBridges([]);
            setSelectedBridge(null);
            setSolution('');
            setCaptcha(null);
            setError(null);

            (async () => {
                try {
                    setLoading(true);
                    const result = await fetchCaptcha();
                    setCaptcha(result);
                } catch (err) {
                    console.error(err);
                    setError('Failed to fetch CAPTCHA. Please try again later.');
                } finally {
                    setLoading(false);
                }
            })();
        }
    }, [open]);

    async function handleCheckSolution() {
        if (!captcha || !solution) return;

        try {
            setLoading(true);
            setError(null);
            const result = await checkCaptcha(captcha.challenge, solution);
            setBridges(result);
            setSelectedBridge(result[0]); // Select first bridge by default
        } catch (err) {
            console.error(err);
            setError(err instanceof Error ? err.message : 'Failed to verify CAPTCHA');
        } finally {
            setLoading(false);
        }
    }

    function handleUseBridge() {
        if (selectedBridge) {
            onSubmit(selectedBridge);
            onClose();
        }
    }

    return (
        <Dialog open={open} maxWidth="sm" fullWidth onClose={onClose}>
            <DialogContent>
                <Box className={classes.container}>
                    <Typography variant="h6">
                        Request Bridge
                    </Typography>

                    {error && (
                        <Typography className={classes.error}>
                            {error}
                        </Typography>
                    )}

                    {loading && !bridges.length && (
                        <Box display="flex" justifyContent="center">
                            <CircularProgress />
                        </Box>
                    )}

                    {!bridges.length && !loading && captcha && (
                        <>
                            <img
                                src={`data:image/jpeg;base64,${captcha.image}`}
                                alt="CAPTCHA"
                                className={classes.captchaImage}
                            />
                            <TextField
                                fullWidth
                                variant="outlined"
                                label="CAPTCHA Solution"
                                value={solution}
                                onChange={(e) => setSolution(e.target.value)}
                                disabled={loading}
                            />
                            <Button
                                variant="contained"
                                color="primary"
                                onClick={handleCheckSolution}
                                disabled={!solution || loading}
                                fullWidth
                            >
                                {loading ? <CircularProgress size={24} /> : 'Submit'}
                            </Button>
                        </>
                    )}

                    {bridges.length > 0 && (
                        <RadioGroup
                            value={selectedBridge}
                            onChange={(e) => setSelectedBridge(e.target.value)}
                            className={classes.radioGroup}
                        >
                            {bridges.map((bridge, index) => (
                                <FormControlLabel
                                    key={index}
                                    value={bridge}
                                    className={classes.bridgeOption}
                                    control={
                                        <Radio color="primary" />
                                    }
                                    label={
                                        <Typography className={classes.bridgeText}>
                                            {bridge}
                                        </Typography>
                                    }
                                />
                            ))}
                        </RadioGroup>
                    )}

                    <DialogActions style={{ padding: 0 }}>
                        <Button onClick={onClose}>
                            Cancel
                        </Button>
                        {bridges.length > 0 && (
                            <Button
                                variant="contained"
                                color="primary"
                                onClick={handleUseBridge}
                                disabled={!selectedBridge}
                            >
                                Use Selected Bridge
                            </Button>
                        )}
                    </DialogActions>
                </Box>
            </DialogContent>
        </Dialog>
    );
}