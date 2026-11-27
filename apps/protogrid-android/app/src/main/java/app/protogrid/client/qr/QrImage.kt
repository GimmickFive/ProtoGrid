package app.protogrid.client.qr

import android.content.Context
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.Matrix
import android.net.Uri
import com.google.zxing.BinaryBitmap
import com.google.zxing.DecodeHintType
import com.google.zxing.RGBLuminanceSource
import com.google.zxing.common.HybridBinarizer
import com.google.zxing.qrcode.QRCodeReader

/** Decodes a QR code from an image picked in the gallery/file picker. */
object QrImage {
    private const val MAX_SIDE = 1600

    fun decode(context: Context, uri: Uri): String? {
        val original = context.contentResolver.openInputStream(uri)?.use {
            BitmapFactory.decodeStream(it)
        } ?: return null
        val bitmap = scaleDown(original)
        val reader = QRCodeReader()
        val hints = mapOf(DecodeHintType.TRY_HARDER to true)
        for (rotation in listOf(0, 90, 180, 270)) {
            val frame = rotate(bitmap, rotation)
            val pixels = IntArray(frame.width * frame.height)
            frame.getPixels(pixels, 0, frame.width, 0, 0, frame.width, frame.height)
            val source = RGBLuminanceSource(frame.width, frame.height, pixels)
            val text = runCatching {
                reader.decode(BinaryBitmap(HybridBinarizer(source)), hints).text?.trim()
            }.getOrNull()
            if (!text.isNullOrEmpty()) return text
        }
        return null
    }

    private fun scaleDown(bitmap: Bitmap): Bitmap {
        val side = maxOf(bitmap.width, bitmap.height)
        if (side <= MAX_SIDE) return bitmap
        val ratio = MAX_SIDE.toFloat() / side
        return Bitmap.createScaledBitmap(
            bitmap,
            (bitmap.width * ratio).toInt().coerceAtLeast(1),
            (bitmap.height * ratio).toInt().coerceAtLeast(1),
            true,
        )
    }

    private fun rotate(bitmap: Bitmap, degrees: Int): Bitmap {
        if (degrees == 0) return bitmap
        val matrix = Matrix().apply { postRotate(degrees.toFloat()) }
        return Bitmap.createBitmap(bitmap, 0, 0, bitmap.width, bitmap.height, matrix, true)
    }
}
