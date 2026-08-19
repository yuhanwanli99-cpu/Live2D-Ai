"""
Vision handler for GLM-4.6V screenshot analysis pipeline.

Captures screenshots, sends them to a vision-capable LLM (e.g. Zhipu GLM-4V),
and returns a text description of the screen content for conversational context.
"""

import base64
import io
from typing import Optional

from openai import AsyncOpenAI, APIError
from loguru import logger
from PIL import Image, ImageGrab


class VisionHandler:
    """
    Handles screenshot capture and vision API calls.

    Uses PIL.ImageGrab for cross-platform screenshot capture and an
    OpenAI-compatible client to send images to a vision model.

    The handler is fully optional — if initialization fails or an API call errors,
    the conversation continues without vision context.
    """

    def __init__(
        self,
        api_key: str,
        model: str = "glm-4v",
        base_url: str = "https://open.bigmodel.cn/api/paas/v4/",
    ):
        """
        Initialize the vision handler.

        Args:
            api_key: API key for the vision provider (Zhipu).
            model: Vision model name (default: glm-4v).
            base_url: Base URL for the OpenAI-compatible API endpoint.
        """
        self._model = model
        self._client = AsyncOpenAI(api_key=api_key, base_url=base_url)
        logger.info(f"VisionHandler initialized with model={model}")

    async def capture_and_describe(
        self,
        prompt: str = "Describe what you see on this screen in detail, focusing on any applications, windows, content, or activities visible.",
    ) -> Optional[str]:
        """
        Capture a screenshot and get a text description from the vision model.

        Args:
            prompt: Text prompt to guide the vision model's description.

        Returns:
            A text description of the screen content, or None if capture or
            API call fails.
        """
        # 1. Capture screenshot
        image_b64 = self._capture_screenshot()
        if image_b64 is None:
            logger.warning("Screenshot capture failed, skipping vision.")
            return None

        # 2. Call vision API
        description = await self._call_vision_api(image_b64, prompt)
        return description

    def _capture_screenshot(self) -> Optional[str]:
        """
        Capture a screenshot and return it as a base64-encoded JPEG string.

        Returns:
            Base64-encoded JPEG string (including data URI prefix), or None on failure.
        """
        try:
            screenshot = ImageGrab.grab()
            # Resize large screenshots to reduce API costs and latency
            # Cap the longest dimension at 1280px while maintaining aspect ratio
            max_dim = 1280
            w, h = screenshot.size
            if max(w, h) > max_dim:
                ratio = max_dim / max(w, h)
                new_w = int(w * ratio)
                new_h = int(h * ratio)
                screenshot = screenshot.resize((new_w, new_h), Image.LANCZOS)

            buffer = io.BytesIO()
            screenshot.convert("RGB").save(buffer, format="JPEG", quality=85)
            b64_str = base64.b64encode(buffer.getvalue()).decode("utf-8")
            logger.debug(
                f"Screenshot captured: {w}x{h} -> {screenshot.size[0]}x{screenshot.size[1]}, "
                f"base64 length={len(b64_str)}"
            )
            return b64_str
        except Exception as e:
            logger.error(f"Failed to capture screenshot: {e}")
            return None

    async def _call_vision_api(
        self,
        image_b64: str,
        prompt: str,
    ) -> Optional[str]:
        """
        Send a screenshot image to the vision model and get a description.

        Args:
            image_b64: Base64-encoded JPEG image string (without data URI prefix).
            prompt: Text prompt describing what the model should look for.

        Returns:
            Description text from the vision model, or None on error.
        """
        data_uri = f"data:image/jpeg;base64,{image_b64}"
        try:
            response = await self._client.chat.completions.create(
                model=self._model,
                messages=[
                    {
                        "role": "user",
                        "content": [
                            {"type": "text", "text": prompt},
                            {
                                "type": "image_url",
                                "image_url": {"url": data_uri, "detail": "high"},
                            },
                        ],
                    }
                ],
                max_tokens=512,
                temperature=0.7,
            )

            description = response.choices[0].message.content or ""
            if description:
                logger.debug(
                    f"Vision description ({len(description)} chars): {description[:120]}..."
                )
            else:
                logger.warning("Vision model returned empty description.")

            return description

        except APIError as e:
            logger.error(f"Vision API error: {e}")
            return None
        except Exception as e:
            logger.error(f"Unexpected vision API error: {e}")
            return None

    async def close(self):
        """Clean up the vision handler's async client."""
        try:
            await self._client.close()
            logger.debug("VisionHandler client closed.")
        except Exception as e:
            logger.warning(f"Error closing VisionHandler client: {e}")
