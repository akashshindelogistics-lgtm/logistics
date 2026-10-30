package com.logistics.driver.data

import androidx.room.Room
import androidx.test.core.app.ApplicationProvider
import androidx.test.ext.junit.runners.AndroidJUnit4
import com.logistics.driver.domain.Fix
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.test.runTest
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith

/**
 * The real Room queue (SQL, not the in-memory fake the JVM tests use), on an
 * in-memory database so each test starts empty.
 */
@RunWith(AndroidJUnit4::class)
class FixDaoTest {
    private lateinit var db: AppDb
    private lateinit var queue: RoomFixQueue

    @Before
    fun open() {
        db = Room.inMemoryDatabaseBuilder(ApplicationProvider.getApplicationContext(), AppDb::class.java).build()
        queue = RoomFixQueue(db.fixDao())
    }

    @After
    fun close() = db.close()

    /** A fix told apart from the others by [recordedAt]. */
    private fun fix(recordedAt: Long) = Fix(18.52, 73.85, recordedAt, accuracyM = 8.0, speedMps = 5.1)

    @Test
    fun enqueueRoundTripsEveryField() = runTest {
        queue.enqueue(Fix(18.7606, 73.8636, 1791400000, accuracyM = 4.5, speedMps = 12.25), cap = 10)
        queue.enqueue(Fix(-33.9, 151.2, 1791400030, accuracyM = null, speedMps = null), cap = 10)

        val (first, second) = queue.oldest(10)

        assertEquals(Fix(18.7606, 73.8636, 1791400000, 4.5, 12.25), first.toFix())
        assertNull(second.accuracyM)
        assertNull(second.speedMps)
    }

    @Test
    fun enqueuePastTheCapDropsTheOldest() = runTest {
        for (t in 1L..5L) queue.enqueue(fix(t), cap = 3)

        assertEquals(3, queue.size.first())
        assertEquals(listOf(3L, 4L, 5L), queue.oldest(10).map { it.recordedAt })
    }

    @Test
    fun oldestReturnsTheOldestFirstUpToTheLimit() = runTest {
        for (t in 1L..250L) queue.enqueue(fix(t), cap = 2000)

        val batch = queue.oldest(100)

        assertEquals(100, batch.size)
        assertEquals((1L..100L).toList(), batch.map { it.recordedAt })
    }

    @Test
    fun removeDeletesOnlyTheUploadedBatch() = runTest {
        for (t in 1L..3L) queue.enqueue(fix(t), cap = 2000)
        val batch = queue.oldest(2)
        // Queued while the batch was uploading; must survive its removal.
        queue.enqueue(fix(4), cap = 2000)

        queue.remove(batch.map { it.id })

        assertEquals(listOf(3L, 4L), queue.oldest(10).map { it.recordedAt })
        assertEquals(2, queue.size.first())
    }

    @Test
    fun removeWithNoIdsAndClear() = runTest {
        for (t in 1L..3L) queue.enqueue(fix(t), cap = 2000)

        queue.remove(emptyList())
        assertEquals(3, queue.size.first())

        queue.clear()
        assertEquals(0, queue.size.first())
        assertEquals(emptyList<QueuedFix>(), queue.oldest(10))
    }
}
