from discord import VoiceProtocol, Client, Intents
from websockets import connect
from dotenv import load_dotenv

import asyncio
from logging import getLogger
from typing import TypedDict
import json
from os import getenv
import base64

import pytest


logger = getLogger(__name__)


class VoiceConnectionData(TypedDict):
    token: str
    endpoint: str
    session_id: str
    guild_id: str
    user_id: str


class VoiceClient(VoiceProtocol):
    def __init__(self, client, channel):
        super().__init__(client, channel)
        self.client = client
        self.channel = channel
        self.guild = channel.guild
        self.waiting_voice_state_update = asyncio.Event()
        self.waiting_voice_server_update = asyncio.Event()
        self._voice_connection_data: VoiceConnectionData = {
            "user_id": str(self.client.user.id),
        }
        self.ws = None
        self.voice_state = {}
        self.voice_server = {}

    async def voice_connect(self):
        await self.guild.change_voice_state(channel=self.channel)

    async def voice_disconnect(self):
        await self.guild.change_voice_state(channel=None)

    async def connect(self, *args, **kwargs):
        logger.info("connnecting")
        await self.voice_connect()
        await self.waiting_voice_state_update.wait()
        await self.waiting_voice_server_update.wait()
        self.ws = await connect("ws://localhost:8000/ws")
        await self.recv()
        logger.info("connected")
        asyncio.create_task(self.recv())

    async def recv(self):
        data = await self.ws.recv()
        data = json.loads(data)
        logger.info(data)
        if data["t"] == "hello":
            await self.ws.send(json.dumps({
                "t": "voice.connection.data",
                "d": self._voice_connection_data,
            }))
        elif data["t"] == "ready":
            logger.info("ready")
            with open("tests/audio.mp3", "rb") as f:
                await self.ws.send(
                    json.dumps({
                        "t": "voice.play",
                        "d": base64.b64encode(f.read()).decode(),
                    })
                )

    async def on_voice_state_update(self, data):
        self.waiting_voice_state_update.set()
        self._voice_connection_data["session_id"] = data.get("session_id")

    async def on_voice_server_update(self, data):
        self.waiting_voice_server_update.set()
        self._voice_connection_data["token"] = data.get("token")
        self._voice_connection_data["endpoint"] = data.get("endpoint")
        self._voice_connection_data["guild_id"] = data.get("guild_id")


@pytest.mark.asyncio
async def test_voice():
    load_dotenv()
    intents = Intents.default()
    intents.voice_states = True
    client = Client(intents=intents)

    @client.event
    async def on_ready():
        logger.info("ready")
        channel = client.get_channel(759362341073715203)
        await channel.connect(cls=VoiceClient)

        await asyncio.sleep(5)
        await client.close()
        

    async with client:
        await client.start(getenv("DISCORD_TOKEN"))